---
title: Getting Started
author: John Smith
tags: [rust, cli, tools]
---

# Setup

This guide covers setting up sigil on your system.

## Prerequisites

You need the following tools:

- Rust 1.70+
- Git 2.30+
- A terminal emulator

## Steps

Run the following command:

```bash
cargo install sigil
```

Then verify the installation:

```python
import subprocess
result = subprocess.run(["sigil", "--version"], capture_output=True)
print(result.stdout)
```

## Troubleshooting

If installation fails, check your Rust toolchain version.

## Configuration

| Option | Default |
|--------|---------|
| verbose | false |

> Important: Always configure sigil before first use
> to ensure optimal performance.

# Usage

Write your first diff command:

```bash
sigil diff HEAD~1
```

## Advanced Usage

For comparing specific files:

1. Choose the old version
2. Choose the new version
3. Run the comparison

Inline code and **bold** text are part of paragraphs.

This is a new paragraph added in v2.
