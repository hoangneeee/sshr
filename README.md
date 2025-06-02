# SSHR

- SSHR là một ứng dụng TUI (Text User Interface) để quản lý và kết nối với các máy chủ thông qua giao diện terminal.

##  UI

![image](./docs/preview_1.png)

## Get started
- `sshr` sẽ đọc file host từ ~/.ssh/config

## Cài đặt

### For Customer

```bash
curl -L -O https://github.com/hoangneeee/sshr/releases/download/v0.1.0/sshr-x86_64-apple-darwin.tar.gz
# or
wget https://github.com/hoangneeee/sshr/releases/download/v0.1.0/sshr-x86_64-apple-darwin.tar.gz

# Unzip
tar -xvf sshr-x86_64-apple-darwin.tar.gz

# Copy to /usr/local/bin
sudo cp sshr-x86_64-apple-darwin/sshr /usr/local/bin/sshr
```

### For Developer
```bash
git clone https://github.com/hoangneeee/sshr.git

cd sshr

cargo build --release
```

## Contribute

- Nếu bạn muốn đóng góp vào dự án này, hãy fork repository này và tạo pull request.
- Nếu bạn muốn báo lỗi hoặc đề xuất cải tiến, hãy tạo issue.