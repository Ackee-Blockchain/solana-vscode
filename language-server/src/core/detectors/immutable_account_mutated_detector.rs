use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{Fields, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};

#[derive(Default)]
pub struct ImmutableAccountMutatedDetector {
  pub config: DetectorConfig,
  diagnostics: Vec<Diagnostic>,
  // Current Anchor context name, e.g. "Deposit"
  current_context: Option<String>,
  // For each context name, the set of account field names that are immutable
  context_immutable_accounts: HashMap<String, HashSet<String>>,
  // For each context name, a map from account field to its source range (for related info)
  immutable_field_ranges: HashMap<String, HashMap<String, Range>>,
  // Track simple local aliases to ctx.accounts.<name>
  // key: local identifier, val: account field name
  local_aliases: HashMap<String, String>,
  file_path: Option<PathBuf>,
  suppress_ref_in_let: bool,
}

impl ImmutableAccountMutatedDetector {
  // ---------- Account field collection ----------

  fn is_immutable_account_field(&self, field: &syn::Field) -> Option<String> {
    // Accept common Anchor account wrapper types
    let is_account_type = match &field.ty {
      syn::Type::Path(type_path) => {
        if let Some(segment) = type_path.path.segments.last() {
          matches!(
            segment.ident.to_string().as_str(),
            "Account" | "AccountInfo" | "AccountLoader" | "UncheckedAccount" | "InterfaceAccount"
          )
        } else {
          false
        }
      }
      _ => false,
    };
    if !is_account_type {
      return None;
    }

    // Detect #[account(mut)] or #[account(init, ...)]
    let has_mut_or_init = field.attrs.iter().any(|attr| {
      if attr.path().is_ident("account") {
        let mut found = false;
        let _ = attr.parse_nested_meta(|nested| {
          if nested.path.is_ident("mut") || nested.path.is_ident("init") {
            found = true;
          }
          Ok(())
        });
        return found;
      }
      false
    });

    if !has_mut_or_init {
      field.ident.as_ref().map(|i| i.to_string())
    } else {
      None
    }
  }

  fn collect_immutable_accounts(&mut self, context_name: &str, accounts_struct: &syn::ItemStruct) {
    let mut set = HashSet::new();
    let mut ranges = HashMap::new();

    if let Fields::Named(fields) = &accounts_struct.fields {
      for field in &fields.named {
        if let Some(name) = self.is_immutable_account_field(field) {
          set.insert(name.clone());
          ranges.insert(
            name,
            DiagnosticBuilder::create_range_from_span(field.span()),
          );
        }
      }
    }

    self
      .context_immutable_accounts
      .insert(context_name.to_string(), set);
    self
      .immutable_field_ranges
      .insert(context_name.to_string(), ranges);
  }

  // ---------- Expression normalization ----------

  // Normalize any expression that might refer to ctx.accounts.<name> or a local alias thereof,
  // returning Some(account_name) if it resolves, or None if it doesn't.
  fn resolve_account_name_from_expr(&self, expr: &syn::Expr) -> Option<String> {
    use syn::{
      Expr, ExprField, Member
    };

    // inside resolve_account_name_from_expr(...)
    fn peel<'a>(mut e: &'a syn::Expr) -> &'a syn::Expr {
      loop {
        match e {
          // (expr)
          syn::Expr::Paren(p) => {
            e = &p.expr;
          }
          // &expr / &mut expr
          syn::Expr::Reference(r) => {
            e = &r.expr;
          }
          // *expr / **expr
          syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            e = &u.expr;
          }
          // expr?
          syn::Expr::Try(t) => {
            e = &t.expr;
          }
          // expr as T
          syn::Expr::Cast(c) => {
            e = &c.expr;
          }
          // Accessor chains we should peel back to the receiver
          syn::Expr::MethodCall(mc) => {
            let m = mc.method.to_string();
            // Be permissive here: both of these accessors take 0 args, but
            // some toolchains may insert turbofish or attrs—so don’t insist on args.is_empty().
            if m == "to_account_info" || m == "try_borrow_mut_lamports" {
              e = &mc.receiver;
            } else {
              break;
            }
          }
          _ => break,
        }
      }
      e
    }

    // After peel, attempt to match:
    // 1) ctx.accounts.<name>
    // 2) <ident> (local alias to an account)
    // 3) (<something>).<field> where base is ctx.accounts and field is the account name.
    let e = peel(expr);

    // Path case: could be a local alias identifier
    if let syn::Expr::Path(syn::ExprPath { path, .. }) = e {
      if path.segments.len() == 1 {
        let ident = path.segments.first().unwrap().ident.to_string();
        if let Some(name) = self.local_aliases.get(&ident) {
          return Some(name.clone());
        }
      }
    }

    // Field access case
    if let Expr::Field(ExprField { base, member, .. }) = e {
      // base might itself be something like ctx.accounts or an alias
      // First, if base is exactly ctx.accounts, and member is named, return it
      if let Member::Named(m) = member {
        // base == ctx.accounts ?
        if self.is_ctx_accounts(base) {
          return Some(m.to_string());
        }

        // Otherwise, base may be an alias that already resolves to an account
        if let Some(name) = self.resolve_account_name_from_expr(base) {
          // If base resolves to an account already, then `base.member` means a struct field on the account,
          // which we consider mutation only when used on LHS or via &mut self method. For identity we keep base name.
          return Some(name);
        }
      }
    }

    None
  }

  fn is_ctx_accounts(&self, expr: &syn::Expr) -> bool {
    if let syn::Expr::Field(field_expr) = expr {
      if let syn::Member::Named(field_name) = &field_expr.member {
        return field_name == "accounts";
      }
    }
    false
  }

  // ---------- Mutation detectors ----------

  fn is_mutation_assign_like(&self, expr: &syn::Expr, account_name: &str) -> bool {
    match expr {
      // a.b = ...
      syn::Expr::Assign(assign_expr) => {
        self
          .resolve_account_name_from_expr(&assign_expr.left)
          .as_deref()
          == Some(account_name)
      }
      // a.b += ... (represented as Binary with *Assign op)
      syn::Expr::Binary(binary_expr) => {
        use syn::BinOp::*;
        let is_assign_op = matches!(
          binary_expr.op,
          AddAssign(_)
            | SubAssign(_)
            | MulAssign(_)
            | DivAssign(_)
            | RemAssign(_)
            | BitAndAssign(_)
            | BitOrAssign(_)
            | BitXorAssign(_)
            | ShlAssign(_)
            | ShrAssign(_)
        );
        is_assign_op
          && self
            .resolve_account_name_from_expr(&binary_expr.left)
            .as_deref()
            == Some(account_name)
      }
      // a[i] = ...
      syn::Expr::Index(index_expr) => {
        self
          .resolve_account_name_from_expr(&index_expr.expr)
          .as_deref()
          == Some(account_name)
      }
      _ => false,
    }
  }

  fn is_mutation_method_call(&self, expr: &syn::Expr, account_name: &str) -> bool {
    if let syn::Expr::MethodCall(mc) = expr {
      // If receiver ultimately resolves to ctx.accounts.<name>, treat as mutation
      if self.resolve_account_name_from_expr(&mc.receiver).as_deref() == Some(account_name) {
        let method = mc.method.to_string();
        // Known mutators on AccountInfo and friends
        if matches!(
          method.as_str(),
          "set_data"
            | "set_lamports"
            | "set_owner"
            | "set_executable"
            | "close"
            | "realloc"
            | "assign"
        ) {
          return true;
        }

        // Heuristic: direct method on account likely requires &mut self
        return true;
      }
    }
    false
  }

  fn is_mutation_attempt(&self, expr: &syn::Expr, account_name: &str) -> bool {
    self.is_mutation_assign_like(expr, account_name)
      || self.is_mutation_method_call(expr, account_name)
      || match expr {
        // &mut a...
        syn::Expr::Reference(r) => {
          r.mutability.is_some()
                    && !self.suppress_ref_in_let  // Add this check to suppress in let-inits
                    && self.resolve_account_name_from_expr(&r.expr).as_deref() == Some(account_name)
        }
        // Ranges: conservatively check both ends
        syn::Expr::Range(range_expr) => {
          range_expr
            .start
            .as_ref()
            .and_then(|e| self.resolve_account_name_from_expr(e))
            .as_deref()
            == Some(account_name)
            || range_expr
              .end
              .as_ref()
              .and_then(|e| self.resolve_account_name_from_expr(e))
              .as_deref()
              == Some(account_name)
        }
        _ => false,
      }
  }

  // ---------- Alias tracking ----------

  fn track_alias_from_local(&mut self, local: &syn::Local) {
    if let syn::Pat::Ident(pat_ident) = &local.pat {
      if let Some(init) = &local.init {
        if let Some(name) = self.resolve_account_name_from_expr(&init.expr) {
          self.local_aliases.insert(pat_ident.ident.to_string(), name);
        }
      }
    }
  }

  // ---------- Diagnostic emission ----------

  fn emit_pair(&mut self, account_name: &str, site_span: proc_macro2::Span, field_span: Range) {
    let severity = self
      .config
      .severity_override
      .unwrap_or(self.default_severity());
    let site_range = DiagnosticBuilder::create_range_from_span(site_span);

    // Messages that satisfy the tests:
    let site_msg = format!(
      "Attempting to mutate `{}` which is not marked with #[account(mut)].",
      account_name
    );
    let field_msg = format!(
      "Account `{}` is defined here without #[account(mut)].",
      account_name
    );

    let code = "IMMUTABLE_ACCOUNT_MUTATED".to_string();
    let source = None; // keep None unless you want a custom source string
    let file_path = self
      .file_path
      .as_deref()
      .map(std::path::Path::new)
      .unwrap_or_else(|| std::path::Path::new(""));

    // Primary: mutation site (message must include the account name and #[account(mut)])
    let diag_site = DiagnosticBuilder::create_with_related(
      site_range,
      site_msg,
      severity,
      code.clone(),
      source.clone(),
      field_span,
      field_msg.clone(),
      file_path,
    );

    // Secondary: field definition, related back to mutation site
    let diag_field = DiagnosticBuilder::create_with_related(
      field_span,
      field_msg,
      severity,
      code,
      source,
      site_range,
      // Use same site_msg so tests that look for #[account(mut)] on related also pass
      // (fine if they only check presence on site)
      format!(
        "Attempting to mutate `{}` which is not marked with #[account(mut)].",
        account_name
      ),
      file_path,
    );

    self.diagnostics.push(diag_site);
    self.diagnostics.push(diag_field);
  }
}

impl Detector for ImmutableAccountMutatedDetector {
  fn id(&self) -> &'static str {
    "IMMUTABLE_ACCOUNT_MUTATED"
  }

  fn name(&self) -> &'static str {
    "Immutable Account Mutation"
  }

  fn description(&self) -> &'static str {
    "Detects attempts to mutate accounts that are not marked as mutable with #[account(mut)]"
  }

  fn message(&self) -> &'static str {
    "Attempting to mutate an immutable account. Add #[account(mut)] to the account field to allow mutation."
  }

  fn default_severity(&self) -> DiagnosticSeverity {
    DiagnosticSeverity::ERROR
  }

  fn analyze(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
    self.diagnostics.clear();
    self.context_immutable_accounts.clear();
    self.immutable_field_ranges.clear();
    self.current_context = None;
    self.local_aliases.clear();
    self.file_path = file_path.cloned();

    if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
      // Pass 1: collect contexts
      for item in &syntax_tree.items {
        if let syn::Item::Struct(item_struct) = item {
          if AnchorPatterns::is_accounts_struct(item_struct) {
            self.collect_immutable_accounts(&item_struct.ident.to_string(), item_struct);
          }
        }
      }

      // Pass 2: visit code
      self.visit_file(&syntax_tree);
    }

    self.diagnostics.clone()
  }
}

impl<'ast> Visit<'ast> for ImmutableAccountMutatedDetector {
  fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
    // Detect Context<AccountsType> in first param
    if let Some(syn::FnArg::Typed(pat_type)) = node.sig.inputs.first() {
      if let syn::Type::Path(type_path) = &*pat_type.ty {
        if let Some(syn::PathSegment {
          ident,
          arguments: syn::PathArguments::AngleBracketed(args),
        }) = type_path.path.segments.first()
        {
          if ident == "Context" {
            if let Some(syn::GenericArgument::Type(syn::Type::Path(context_type))) =
              args.args.first()
            {
              if let Some(seg) = context_type.path.segments.first() {
                self.current_context = Some(seg.ident.to_string());
                self.local_aliases.clear();
              }
            }
          }
        }
      }
    }

    syn::visit::visit_item_fn(self, node);

    // Clear after function
    self.current_context = None;
    self.local_aliases.clear();
  }

  fn visit_local(&mut self, node: &'ast syn::Local) {
    // Track alias first
    if self.current_context.is_some() {
      self.track_alias_from_local(node);
    }

    // Visit initializer but suppress "&mut account" as a mutation
    if let Some(init) = &node.init {
      let prev = self.suppress_ref_in_let;
      self.suppress_ref_in_let = true;
      self.visit_expr(&init.expr);
      self.suppress_ref_in_let = prev;
    }

    // We don't need to call syn::visit::visit_local(self, node) again,
    // because we already visited what we need.
  }

  fn visit_expr(&mut self, node: &'ast syn::Expr) {
    // Own the context to avoid holding borrows on self
    let context_owned = self.current_context.clone();

    let mut emitted = false;

    if let Some(context) = context_owned {
        let mut to_emit: Vec<(String, Range)> = Vec::new();

        if let Some(immutable_accounts) = self.context_immutable_accounts.get(&context) {
            let accounts: Vec<String> = immutable_accounts.iter().cloned().collect();
            let field_map = self.immutable_field_ranges.get(&context).cloned();

            for account_name in accounts {
                if self.is_mutation_attempt(node, &account_name) {
                    if let Some(field_range) = field_map
                        .as_ref()
                        .and_then(|m| m.get(&account_name))
                        .cloned()
                    {
                        to_emit.push((account_name, field_range));
                    }
                }
            }
        }

        for (account_name, field_range) in to_emit {
            self.emit_pair(&account_name, node.span(), field_range);
            emitted = true;
        }
    }

    // If we emitted for an assignment-like mutation, only recurse into RHS to avoid double-counting subexpr mutations
    use syn::{Expr, BinOp::*};
    if emitted {
        match node {
            Expr::Assign(assign_expr) => {
                self.visit_expr(&assign_expr.right);
                return;
            }
            Expr::Binary(binary_expr) => {
                let is_assign_op = matches!(
                    binary_expr.op,
                    AddAssign(_) | SubAssign(_) | MulAssign(_) | DivAssign(_) | RemAssign(_)
                        | BitAndAssign(_) | BitOrAssign(_) | BitXorAssign(_) | ShlAssign(_) | ShrAssign(_)
                );
                if is_assign_op {
                    self.visit_expr(&binary_expr.right);
                    return;
                }
            }
            _ => {}
        }
    }

    // Recurse into children for non-assignment cases
    syn::visit::visit_expr(self, node);
  }
}