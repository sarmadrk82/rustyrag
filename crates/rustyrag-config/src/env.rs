use rustyrag_core::{Error, Result};

/// Load variables from a `.env` file in the current directory, if present.
/// Existing shell environment variables are kept (`.env` does not override them).
pub fn load_dotenv() {
    match dotenvy::dotenv() {
        Ok(path) => tracing::debug!(path = %path.display(), "loaded .env file"),
        Err(dotenvy::Error::Io(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => tracing::warn!(error = %err, "failed to load .env file"),
    }
}

/// Replace `${VAR}` placeholders with environment variable values.
pub fn substitute_env(input: &str) -> Result<String> {
    let re = regex::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").expect("valid regex");
    let mut output = input.to_string();

    for caps in re.captures_iter(input) {
        let full = caps.get(0).expect("match").as_str();
        let var = caps.get(1).expect("group").as_str();
        let value = std::env::var(var).map_err(|_| {
            Error::Config(format!(
                "environment variable `{var}` is not set (needed for `{full}`). \
                 Set it in your shell or in a `.env` file in the project root."
            ))
        })?;
        output = output.replace(full, &value);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_env_vars() {
        std::env::set_var("RUSTYRAG_TEST_VAR", "hello");
        let out = substitute_env("url: ${RUSTYRAG_TEST_VAR}").unwrap();
        assert_eq!(out, "url: hello");
    }
}
