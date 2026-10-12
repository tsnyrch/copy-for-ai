use crate::{Error, ModelVariant};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf};

const FILES: &[(&str, u64, &str)] = &[
    (
        "model.onnx",
        1_157_129_714,
        "57af7f3d8d72a3ca4e5619c247344b4b5ee362a5ebcf307c0634d67b65e721c0",
    ),
    (
        "tokenizer.json",
        16_331_948,
        "914bd3c8fb7b525af9e23b60d0ec7b1248ddb2b99014efd9c02ebeb022f8cab7",
    ),
];

pub fn model_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("COPY_FOR_AI_MODEL_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(contents) = executable.parent().and_then(|p| p.parent())
    {
        let bundled = contents.join("Resources/models/gliner-multi");
        if bundled.is_dir() {
            return bundled;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/gliner-multi")
}

pub(crate) const TOKENIZER_DIGEST: &str = FILES[1].2;

pub(crate) fn model_spec(variant: ModelVariant) -> (&'static str, u64, &'static str) {
    match variant {
        ModelVariant::Fp32 => FILES[0],
        ModelVariant::Q4 => (
            "model-q4.onnx",
            893_933_650,
            "3c54cf1332d491f7f4057504df4ac78a78712874aca1c5731330430e0c55259b",
        ),
        ModelVariant::Int8 => (
            "model-int8.onnx",
            349_120_924,
            "995058c82c5f570601dd8a0ba74ee60a392f764268bc5f628455e44dd3b476ec",
        ),
    }
}

pub(crate) fn verify(
    directory: &std::path::Path,
    variant: ModelVariant,
    cancelled: &impl Fn() -> bool,
) -> Result<(), Error> {
    let model = model_spec(variant);
    for (name, size, expected) in [model, FILES[1]] {
        let mut file = File::open(directory.join(name)).map_err(|_| Error::Missing)?;
        if file.metadata().map_err(|_| Error::InvalidAssets)?.len() != size {
            return Err(Error::InvalidAssets);
        }
        let mut digest = Sha256::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            let length = file.read(&mut buffer).map_err(|_| Error::InvalidAssets)?;
            if length == 0 {
                break;
            }
            digest.update(&buffer[..length]);
        }
        if format!("{:x}", digest.finalize()) != expected {
            return Err(Error::InvalidAssets);
        }
    }
    Ok(())
}
