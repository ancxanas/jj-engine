//! Test fixtures for generating test projects

use std::fs;
use std::path::Path;

/// Writes a file, creating parent directories if needed
pub fn write_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

/// Default AVCS config for daemon mode with all required fields
pub fn avcs_config_toml(auto_commit: bool) -> String {
    format!(r#"
[project]
name = "test-project"
language = "typescript"

[analysis]
include_patterns = ["src/**/*", "tests/**/*", "docs/**/*", "*.md", "*.json", "*.toml"]
exclude_patterns = ["node_modules/**", "dist/**", ".next/**", "coverage/**"]

[autonomy]
auto_commit_patterns = ["Feature", "BugFix", "TestAddition", "Documentation", "Configuration", "DependencyUpdate"]

[commit]
message_style = "conventional"
include_evidence_footer = true
sign_commits = false
max_subject_length = 72

[output]
default_format = "human"
color = true

[daemon]
enabled = true
debounce_ms = 1000
watch_patterns = ["**/*"]
exclude_patterns = ["target/**", ".git/**", "*.log", "node_modules/**"]
log_level = "debug"
auto_commit = {}

[classifier]
tier1_rules = ["migration", "docker", "ci", "package_lockfile"]
tier2_rules = ["security", "test", "feature", "bugfix", "refactor", "docs", "config", "dependency"]
tier3_rules = ["feature", "bugfix", "refactor", "docs", "config", "dependency"]

"#, auto_commit)
}

/// Creates a toy TypeScript project structure for testing
pub fn create_toy_project(base: &Path) -> anyhow::Result<()> {
    // package.json
    write_file(&base.join("package.json"), r#"{
  "name": "toy-project",
  "version": "1.0.0",
  "description": "A toy project for AVCS testing",
  "main": "dist/index.js",
  "scripts": {
    "build": "tsc",
    "test": "jest"
  },
  "dependencies": {
    "axios": "^1.6.0"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "jest": "^29.7.0",
    "@types/jest": "^29.5.0",
    "@types/node": "^20.10.0"
  }
}
"#)?;

    // tsconfig.json
    write_file(&base.join("tsconfig.json"), r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "tests"]
}
"#)?;

    // src/index.ts
    write_file(&base.join("src/index.ts"), r#"export { add, subtract } from './utils/math';
export { formatName } from './utils/helpers';
export { authenticate, authorize } from './features/auth';
export { processPayment } from './features/payments';
export { settings } from './config/settings';
"#)?;

    // src/utils/math.ts
    write_file(&base.join("src/utils/math.ts"), r#"export function add(a: number, b: number): number {
  return a + b;
}

export function subtract(a: number, b: number): number {
  return a - b;
}

export function multiply(a: number, b: number): number {
  return a * b;
}

export function divide(a: number, b: number): number {
  if (b === 0) throw new Error('Division by zero');
  return a / b;
}
"#)?;

    // src/utils/helpers.ts
    write_file(&base.join("src/utils/helpers.ts"), r#"export function formatName(first: string, last: string): string {
  return `${first} ${last}`.trim();
}

export function capitalize(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1).toLowerCase();
}

export function slugify(str: string): string {
  return str.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
}
"#)?;

    // src/features/auth.ts
    write_file(&base.join("src/features/auth.ts"), r#"export interface User {
  id: string;
  email: string;
  passwordHash: string;
}

export function authenticate(email: string, password: string): Promise<User | null> {
  // Simulate authentication
  return Promise.resolve({ id: '1', email, passwordHash: 'hashed' });
}

export function authorize(user: User, resource: string): boolean {
  // Simple authorization logic
  return true;
}

export function validateInput(input: string): boolean {
  // Basic input validation
  return input.length > 0 && input.length < 100;
}
"#)?;

    // src/features/payments.ts
    write_file(&base.join("src/features/payments.ts"), r#"export interface Payment {
  id: string;
  amount: number;
  currency: string;
  status: 'pending' | 'completed' | 'failed';
}

export async function processPayment(payment: Payment): Promise<Payment> {
  // Simulate payment processing
  return { ...payment, status: 'completed' };
}

export function calculateTax(amount: number, rate: number): number {
  return amount * rate;
}
"#)?;

    // src/config/settings.ts
    write_file(&base.join("src/config/settings.ts"), r#"export const settings = {
  apiUrl: process.env.API_URL || 'http://localhost:3000',
  timeout: 5000,
  retries: 3,
};

export function getConfig() {
  return { ...settings };
}
"#)?;

    // Tests - utils/math.test.ts
    write_file(&base.join("tests/utils/math.test.ts"), r#"import { add, subtract, multiply, divide } from '../../src/utils/math';

describe('Math utilities', () => {
  test('add', () => {
    expect(add(2, 3)).toBe(5);
    expect(add(-1, 1)).toBe(0);
  });

  test('subtract', () => {
    expect(subtract(5, 3)).toBe(2);
  });

  test('multiply', () => {
    expect(multiply(4, 3)).toBe(12);
  });

  test('divide', () => {
    expect(divide(10, 2)).toBe(5);
    expect(() => divide(5, 0)).toThrow('Division by zero');
  });
});
"#)?;

    // Tests - utils/helpers.test.ts
    write_file(&base.join("tests/utils/helpers.test.ts"), r#"import { formatName, capitalize, slugify } from '../../src/utils/helpers';

describe('Helper utilities', () => {
  test('formatName', () => {
    expect(formatName('John', 'Doe')).toBe('John Doe');
    expect(formatName('  John  ', '  Doe  ')).toBe('John Doe');
  });

  test('capitalize', () => {
    expect(capitalize('hello')).toBe('Hello');
    expect(capitalize('WORLD')).toBe('World');
  });

  test('slugify', () => {
    expect(slugify('Hello World')).toBe('hello-world');
    expect(slugify('Test@String!')).toBe('teststring');
  });
});
"#)?;

    // Tests - features/auth.test.ts
    write_file(&base.join("tests/features/auth.test.ts"), r#"import { authenticate, authorize, validateInput } from '../../src/features/auth';

describe('Authentication', () => {
  test('authenticate returns user', async () => {
    const user = await authenticate('test@example.com', 'password');
    expect(user).toBeDefined();
    expect(user?.email).toBe('test@example.com');
  });

  test('authorize returns true', () => {
    const user = { id: '1', email: 'test@example.com', passwordHash: 'hash' };
    expect(authorize(user, 'resource')).toBe(true);
  });

  test('validateInput', () => {
    expect(validateInput('valid')).toBe(true);
    expect(validateInput('')).toBe(false);
    expect(validateInput('a'.repeat(101))).toBe(false);
  });
});
"#)?;

    // Tests - features/payments.test.ts
    write_file(&base.join("tests/features/payments.test.ts"), r#"import { processPayment, calculateTax, Payment } from '../../src/features/payments';

describe('Payments', () => {
  test('processPayment', async () => {
    const payment: Payment = { id: '1', amount: 100, currency: 'USD', status: 'pending' };
    const result = await processPayment(payment);
    expect(result.status).toBe('completed');
  });

  test('calculateTax', () => {
    expect(calculateTax(100, 0.1)).toBe(10);
    expect(calculateTax(50, 0.2)).toBe(10);
  });
});
"#)?;

    // Tests - integration.test.ts
    write_file(&base.join("tests/integration.test.ts"), r#"import { authenticate } from '../src/features/auth';
import { processPayment } from '../src/features/payments';

describe('Integration', () => {
  test('auth then payment flow', async () => {
    const user = await authenticate('test@example.com', 'password');
    expect(user).toBeDefined();
    
    if (user) {
      const payment = await processPayment({ id: '1', amount: 100, currency: 'USD', status: 'pending' });
      expect(payment.status).toBe('completed');
    }
  });
});
"#)?;

    // .github/workflows/ci.yml
    write_file(&base.join(".github/workflows/ci.yml"), r#"name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
      - run: npm run build
      - run: npm test
"#)?;

    // docs/README.md
    write_file(&base.join("docs/README.md"), r#"# Toy Project

A simple toy project for testing AVCS.

## Features

- Authentication
- Payments
- Utilities
"#)?;

    // docs/API.md
    write_file(&base.join("docs/API.md"), r#"# API Documentation

## Authentication

- `authenticate(email, password)` - Authenticate user
- `authorize(user, resource)` - Authorize access

## Payments

- `processPayment(payment)` - Process payment
- `calculateTax(amount, rate)` - Calculate tax
"#)?;

    // docker-compose.yml
    write_file(&base.join("docker-compose.yml"), r#"version: '3.8'

services:
  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - API_URL=http://localhost:3000
"#)?;

    Ok(())
}