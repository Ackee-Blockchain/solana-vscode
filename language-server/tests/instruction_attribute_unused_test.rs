#[cfg(test)]
mod tests {
    use language_server::core::detectors::detector::Detector;
    use language_server::core::detectors::instruction_attribute_unused::InstructionAttributeUnusedDetector;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    #[test]
    fn test_detects_unused_instruction_parameter() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
#[instruction(data_2: u8, parameter2: String, parameter1237y012: bool)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + 32, seeds = [b"hello"], bump)]
    pub account: Account<'info, TestAccount>,
    /// CHECK: ok
    pub program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // All three parameters should be reported as unused
        assert_eq!(diagnostics.len(), 3);

        // Check that all diagnostics are warnings
        for diagnostic in &diagnostics {
            assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        }

        // Check that the messages contain the parameter names
        let messages: Vec<String> = diagnostics.iter().map(|d| d.message.clone()).collect();
        assert!(messages.iter().any(|msg| msg.contains("data_2")));
        assert!(messages.iter().any(|msg| msg.contains("parameter2")));
        assert!(messages.iter().any(|msg| msg.contains("parameter1237y012")));
    }

    #[test]
    fn test_detects_used_instruction_parameter_in_seeds() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
#[instruction(seed_value: u8)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + 32, seeds = [b"hello", seed_value.to_le_bytes().as_ref()], bump)]
    pub account: Account<'info, TestAccount>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // seed_value is used in seeds, so no diagnostic should be reported
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_detects_used_instruction_parameter_in_space() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
#[instruction(data_size: usize)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + data_size)]
    pub account: Account<'info, TestAccount>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // data_size is used in space calculation, so no diagnostic should be reported
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_detects_used_instruction_parameter_in_constraints() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
#[instruction(expected_value: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + 32, constraint = account.value == expected_value)]
    pub account: Account<'info, TestAccount>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // expected_value is used in constraint, so no diagnostic should be reported
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_mixed_used_and_unused_parameters() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
#[instruction(used_param: u8, unused_param: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + 32, seeds = [b"hello", used_param.to_le_bytes().as_ref()], bump)]
    pub account: Account<'info, TestAccount>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Only unused_param should be reported
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("unused_param"));
    }

    #[test]
    fn test_no_instruction_attribute() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + 32)]
    pub account: Account<'info, TestAccount>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // No instruction attribute, so no diagnostics should be reported
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_non_accounts_struct() {
        let mut detector = InstructionAttributeUnusedDetector::default();

        let code = r#"
#[instruction(param: u8)]
pub struct SomeOtherStruct {
    pub field: u8,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Not an Accounts struct, so no diagnostics should be reported
        assert_eq!(diagnostics.len(), 0);
    }
}
