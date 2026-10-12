use copy_for_ai_core::{CustomRule, Detector, Direction, EntityKind, Vault};
use copy_for_ai_ner::{LocalNer, model_directory};
use std::io::{self, BufRead, Read, Write};
use zeroize::Zeroizing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let detector = Detector::new();
    let mut vault = Vault::new();
    let mut model = None;
    let mut rules = Vec::new();
    if std::env::args().any(|arg| arg == "--stdin") {
        let mut bytes = Zeroizing::new(Vec::new());
        io::stdin()
            .lock()
            .take((copy_for_ai_core::MAX_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > copy_for_ai_core::MAX_OUTPUT_BYTES {
            return Err(copy_for_ai_core::Error::InputTooLarge.into());
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Input is not valid UTF-8."))?;
        let entities = if vault.direction(text)? == Direction::Anonymize {
            LocalNer::load(&model_directory())?.detect(text, || false)?
        } else {
            Vec::new()
        };
        let output = vault.process(&detector, text, &[], &entities)?;
        if output.direction == Direction::Unresolved {
            eprintln!("Local marker mappings are missing; processing stopped.");
        } else {
            io::stdout().lock().write_all(output.text.as_bytes())?;
        }
        return Ok(());
    }
    eprintln!("Copy for AI — automatic direction; mappings stay in memory");
    eprintln!("/person NAME; /org COMPANY; /new (keeps sessions); /forget; /quit");
    let mut stdin = io::stdin().lock();
    loop {
        let mut bytes = Zeroizing::new(Vec::new());
        let size = (&mut stdin)
            .take((copy_for_ai_core::MAX_OUTPUT_BYTES + 18) as u64)
            .read_until(b'\n', &mut bytes)?;
        if size == 0 {
            break;
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        if bytes.len() > copy_for_ai_core::MAX_OUTPUT_BYTES + 16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Line exceeds the input limit.",
            )
            .into());
        }
        let line = Zeroizing::new(
            std::str::from_utf8(&bytes)
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Input is not valid UTF-8.")
                })?
                .to_owned(),
        );
        if line.as_str() == "/quit" {
            break;
        }
        if line.as_str() == "/new" {
            match vault.new_session() {
                Ok(()) => eprintln!("New session ready."),
                Err(error) => eprintln!("{error}"),
            }
            continue;
        }
        if line.as_str() == "/forget" {
            vault.forget();
            rules.clear();
            eprintln!("All mappings forgotten.");
            continue;
        }
        if let Some((kind, value)) = line
            .strip_prefix("/person ")
            .map(|v| (EntityKind::Person, v))
            .or_else(|| {
                line.strip_prefix("/org ")
                    .map(|v| (EntityKind::Organization, v))
            })
        {
            if rules.len() == copy_for_ai_core::MAX_CUSTOM_RULES {
                eprintln!("Too many rules.");
                continue;
            }
            match CustomRule::new(kind, value) {
                Ok(rule) => {
                    rules.push(rule);
                    eprintln!("Rule added.");
                }
                Err(error) => eprintln!("{error}"),
            }
            continue;
        }
        let text = line.strip_prefix("/restore ").unwrap_or(&line);
        let direction = match vault.direction(text) {
            Ok(direction) => direction,
            Err(error) => {
                eprintln!("{error}");
                continue;
            }
        };
        let entities = if direction == Direction::Anonymize {
            if model.is_none() {
                match LocalNer::load(&model_directory()) {
                    Ok(loaded) => model = Some(loaded),
                    Err(error) => {
                        eprintln!("{error}");
                        continue;
                    }
                }
            }
            match model.as_ref().expect("model loaded").detect(text, || false) {
                Ok(entities) => entities,
                Err(error) => {
                    eprintln!("{error}");
                    continue;
                }
            }
        } else {
            Vec::new()
        };
        match vault.process(&detector, text, &rules, &entities) {
            Ok(output) if output.direction != Direction::Unresolved => {
                println!("{}", output.text);
                eprintln!(
                    "Occurrences: {}; unknown markers: {}",
                    output.replacements,
                    output.unknown_tokens.len()
                );
            }
            Ok(_) => eprintln!("Local marker mappings are missing; processing stopped."),
            Err(error) => eprintln!("{error}"),
        }
        io::stdout().flush()?;
    }
    Ok(())
}
