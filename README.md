# 💻 SSHR

- SSHR is a TUI (Text User Interface) application for managing and connecting to hosts through the terminal interface.
- Current support only MacOs and Linux (x86_64)

[![Release](https://github.com/hoangneeee/sshr/actions/workflows/release.yml/badge.svg)](https://github.com/hoangneeee/sshr/actions/workflows/release.yml)

## 🖥️ UI

![image](./docs/preview_1.png)

## 🚀 Get started

- `sshr` will load host from ~/.ssh/config

## 📦 Install

### 🍺 Install using Homebrew (recommended)

```bash
brew tap hoangneeee/sshr
brew install sshr
```

### ⬇️ Install from release

```bash
curl -L -O https://github.com/hoangneeee/sshr/releases/download/v0.1.0/sshr-x86_64-apple-darwin.tar.gz
# or
wget https://github.com/hoangneeee/sshr/releases/download/v0.1.0/sshr-x86_64-apple-darwin.tar.gz

# Unzip
tar -xvf sshr-x86_64-apple-darwin.tar.gz

# Copy to /usr/local/bin
sudo cp sshr-x86_64-apple-darwin/sshr /usr/local/bin/sshr
```

### 🔨 For Developer

```bash
git clone https://github.com/hoangneeee/sshr.git

cd sshr

make install
```

## 📝 Available flags

| Flag        | Short flag | Description             |
| ----------- | ---------- | ----------------------- |
| `--version` | `-V`       | Current version of sshr |
| `--help`    | `-h`       | Show help               |

## 🤝 Contribute

- If you want to contribute to this project, please fork this repository and create a pull request.
- If you want to report an issue or suggest an improvement, please create an issue.


## 📝 License

[Apache License 2.0](./LICENSE)
