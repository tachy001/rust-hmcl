//! Input validators, ports of HMCL's `ui.construct` validator classes.

/// Validation result for a text input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validation {
    Valid,
    Invalid,
}

/// Checks performed on text inputs.
pub trait Validator {
    /// Validate `value`, returning the reason for rejection when invalid.
    fn validate(&self, value: &str) -> Result<(), String>;
}

/// Rejects empty input.
pub struct RequiredValidator;

impl Validator for RequiredValidator {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            Err(crate::i18n::tr("input.required"))
        } else {
            Ok(())
        }
    }
}

/// Rejects values that are not finite numbers.
pub struct NumberValidator;

impl Validator for NumberValidator {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.parse::<f64>().is_ok() {
            Ok(())
        } else {
            Err(crate::i18n::tr("input.number"))
        }
    }
}

/// Rejects values that are not valid URLs.
pub struct UrlValidator;

impl Validator for UrlValidator {
    fn validate(&self, value: &str) -> Result<(), String> {
        if url_like(value) {
            Ok(())
        } else {
            Err(crate::i18n::tr("input.url"))
        }
    }
}

/// Whether `value` looks like an absolute http(s) URL.
pub fn url_like(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("http://").or_else(|| value.strip_prefix("https://")) else {
        return false;
    };
    !rest.is_empty() && !rest.chars().any(char::is_whitespace)
}

/// Validate `value` with all `validators`, returning the first error.
pub fn validate_all(validators: &[&dyn Validator], value: &str) -> Result<(), String> {
    for validator in validators {
        validator.validate(value)?;
    }
    Ok(())
}
