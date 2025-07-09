#[cfg(test)]
mod tests {
    use language_server::core::detectors::detector::Detector;
    use language_server::core::detectors::instruction_attribute_invalid::InstructionAttributeInvalidDetector;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    #[test]
    fn test_valid_instruction_attribute_same_order() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String, input_two: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_one: String, input_two: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for valid usage
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_valid_instruction_attribute_partial_params() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String, input_two: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_one: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for valid partial usage
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_invalid_instruction_attribute_wrong_order() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String, input_two: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_two: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error for wrong parameter order
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("input_two"));
        assert!(diagnostics[0].message.contains("input_one"));
        assert!(diagnostics[0].message.contains("same order"));
    }

    #[test]
    fn test_invalid_instruction_attribute_too_many_params() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_one: String, input_two: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error for too many parameters
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("input_two"));
        assert!(diagnostics[0].message.contains("not found in handler"));
    }

    #[test]
    fn test_no_instruction_attribute() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics when no instruction attribute is present
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_non_accounts_struct() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String) -> Result<()> {
        Ok(())
    }
}

#[instruction(input_one: String)]
pub struct SomeOtherStruct {
    pub field: String,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for non-Accounts structs
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_no_matching_handler() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn other_function(ctx: Context<Initialize>, input_one: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_one: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics when no matching handler is found
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_mixed_valid_invalid_params() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, input_one: String, input_two: String, input_three: bool) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(input_one: String, input_three: bool)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error for wrong parameter order (input_three should be after input_two)
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("input_three"));
        assert!(diagnostics[0].message.contains("input_two"));
    }

    #[test]
    fn test_invalid_instruction_attribute_type_mismatch() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, data: u8, text: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(data: u64, text: bool)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report errors for type mismatches
        assert_eq!(diagnostics.len(), 2);

        // Check first error (data: u64 vs u8)
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("data"));
        assert!(diagnostics[0].message.contains("u64"));
        assert!(diagnostics[0].message.contains("u8"));

        // Check second error (text: bool vs u8)
        assert_eq!(diagnostics[1].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[1].message.contains("text"));
        assert!(diagnostics[1].message.contains("bool"));
        assert!(diagnostics[1].message.contains("u8"));
    }

    #[test]
    fn test_valid_instruction_attribute_matching_types() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn initialize(ctx: Context<Initialize>, data: u8, text: bool) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(data: u8, text: bool)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for matching types
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_different_function_name_than_context() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn create_user(ctx: Context<Initialize>, user_name: String, user_age: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(user_name: String, user_age: u8)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics - function name is different but Context<Initialize> matches
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_different_function_name_with_type_mismatch() {
        let mut detector = InstructionAttributeInvalidDetector::default();

        let code = r#"
#[program]
pub mod example {
    use super::*;
    
    pub fn create_user(ctx: Context<Initialize>, user_name: String, user_age: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(user_name: String, user_age: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report type mismatch for user_age (u64 vs u8)
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("user_age"));
        assert!(diagnostics[0].message.contains("u64"));
        assert!(diagnostics[0].message.contains("u8"));
    }
}
