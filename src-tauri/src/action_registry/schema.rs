use super::{ErrorCode, RegistryError, Result};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParameterType {
    Text { max_length: usize },
    Number { min: f64, max: f64, integer: bool },
    Boolean,
    Choice { options: Vec<String> },
    Application,
    File,
    Folder,
}

#[derive(Debug, Clone, Serialize)]
pub struct Parameter {
    pub key: String,
    pub label: String,
    #[serde(flatten)]
    pub kind: ParameterType,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl Parameter {
    pub fn required(key: &str, label: &str, kind: ParameterType) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            kind,
            required: true,
            default: None,
        }
    }
    fn check(&self, value: &Value) -> Result<()> {
        let bounded_text = |limit| {
            value.as_str().is_some_and(|text| {
                !text.trim().is_empty()
                    && text.chars().count() <= limit
                    && !text.chars().any(char::is_control)
            })
        };
        let valid = match &self.kind {
            ParameterType::Text { max_length } => bounded_text(*max_length),
            ParameterType::Number { min, max, integer } => value.as_f64().is_some_and(|number| {
                number.is_finite()
                    && number >= *min
                    && number <= *max
                    && (!integer
                        || (number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0))
            }),
            ParameterType::Boolean => value.is_boolean(),
            ParameterType::Choice { options } => value
                .as_str()
                .is_some_and(|choice| options.iter().any(|option| option == choice)),
            ParameterType::Application => bounded_text(131072),
            ParameterType::File | ParameterType::Folder => bounded_text(32767),
        };
        if valid {
            Ok(())
        } else {
            Err(RegistryError::field(
                &self.key,
                format!(
                    "Invalid value for {}. Use the declared type and constraints.",
                    self.label
                ),
            ))
        }
    }
}

pub(super) fn check_schema(parameters: &[Parameter]) -> Result<()> {
    let mut keys = std::collections::HashSet::new();
    for parameter in parameters {
        if parameter.key.is_empty() || !keys.insert(&parameter.key) {
            return Err(RegistryError::new(
                ErrorCode::InvalidRegistration,
                "Parameter keys must be nonempty and unique.",
            ));
        }
        match &parameter.kind {
            ParameterType::Number { min, max, .. }
                if !min.is_finite() || !max.is_finite() || min > max =>
            {
                return Err(RegistryError::new(
                    ErrorCode::InvalidRegistration,
                    "Invalid number constraints.",
                ));
            }
            ParameterType::Choice { options }
                if options.is_empty()
                    || options.iter().any(String::is_empty)
                    || options
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != options.len() =>
            {
                return Err(RegistryError::new(
                    ErrorCode::InvalidRegistration,
                    "Choice options must be nonempty and unique.",
                ));
            }
            ParameterType::Text { max_length: 0 } => {
                return Err(RegistryError::new(
                    ErrorCode::InvalidRegistration,
                    "Text needs a positive length bound.",
                ));
            }
            _ => {}
        }
        if let Some(default) = &parameter.default {
            parameter.check(default).map_err(|_| {
                RegistryError::new(
                    ErrorCode::InvalidRegistration,
                    "Default does not satisfy parameter schema.",
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn validate(parameters: &[Parameter], input: &Value) -> Result<Map<String, Value>> {
    let object = input
        .as_object()
        .ok_or_else(|| RegistryError::field("parameters", "Parameters must be an object."))?;
    for key in object.keys() {
        if !parameters.iter().any(|parameter| parameter.key == *key) {
            return Err(RegistryError::field(
                key,
                "Unknown parameter. Confirmation and execution policies cannot be supplied as parameters.",
            ));
        }
    }
    let mut output = Map::new();
    for parameter in parameters {
        let value = object.get(&parameter.key).or(parameter.default.as_ref());
        if let Some(value) = value {
            parameter.check(value)?;
            // Canonicalize integral numbers so native integer arguments never depend
            // on whether JSON spelled a value as 35 or 35.0.
            let value = if matches!(parameter.kind, ParameterType::Number { integer: true, .. }) {
                Value::from(value.as_f64().unwrap() as i64)
            } else {
                value.clone()
            };
            output.insert(parameter.key.clone(), value);
        } else if parameter.required {
            return Err(RegistryError::field(
                &parameter.key,
                format!("{} is required.", parameter.label),
            ));
        }
    }
    Ok(output)
}
