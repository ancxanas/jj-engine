//! Toy project fixture generator

use std::path::Path;
use crate::common::fixture::{write_file, create_toy_project};

/// Sets up a toy project in the given directory
pub fn setup_toy_project(base: &Path) -> anyhow::Result<()> {
    create_toy_project(base)
}

/// Creates a minimal toy project with just essential files
pub fn create_minimal_toy_project(base: &Path) -> anyhow::Result<()> {
    // package.json
    write_file(&base.join("package.json"), r#"{
  "name": "minimal-toy",
  "version": "1.0.0",
  "scripts": {
    "test": "jest"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "jest": "^29.0.0",
    "@types/jest": "^29.0.0"
  }
}
"#)?;

    // tsconfig.json
    write_file(&base.join("tsconfig.json"), r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true,
    "outDir": "./dist"
  },
  "include": ["src/**/*"]
}
"#)?;

    // src/index.ts
    write_file(&base.join("src/index.ts"), "export const hello = 'world';")?;

    // tests/index.test.ts
    write_file(&base.join("tests/index.test.ts"), "test('hello', () => { expect(true).toBe(true); });")?;

    // docs/README.md
    write_file(&base.join("docs/README.md"), "# Minimal Toy\n")?;

    Ok(())
}