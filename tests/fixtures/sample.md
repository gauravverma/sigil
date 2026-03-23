---
title: Getting Started
author: Jane Doe
tags: [rust, cli]
---

# Installation

This guide covers installing sigil on your system.

## Prerequisites

You need the following tools:

- Rust 1.70+
- Git 2.30+
- A terminal emulator

## Steps

Run the following command:

```bash
curl -sSf https://example.com/install.sh | sh
```

Then verify the installation:

```python
import subprocess
result = subprocess.run(["sigil", "--version"], capture_output=True)
print(result.stdout)
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| verbose | false | Enable verbose output |
| color | true | Enable colored output |

> Note: Configuration is optional. Sigil works out of the box
> with sensible defaults for most use cases.

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
