# Mục tiêu dự án sshr

- Sử dụng Rust
- Tạo một công cụ dòng lệnh giao diện TUI (giống lazygit) để:
  • Quản lý danh sách các SSH host (host, user, port, alias, mô tả…)
  • Chọn nhanh một host để kết nối qua ssh
  • Dùng phím điều hướng để connect, thêm, sửa, xoá host
  • (Có thể) Tích hợp SCP, phân loại host theo nhóm

# Gợi ý tech stack (Rust)

# Thành phần:

- TUI: Gợi ý thư viện ratatui hoặc tui, Ghi chú: UI đẹp linh hoạt
- Terminal input: crossterm, Đọc input bàn phím đa nền tảng
- Config storage: serde, serde_json, toml, Để lưu danh sách SSH vào file
- Command exec: std::process::Command hoặc duct, Để gọi ssh, scp, v.v.
- Clipboard: arboard hoặc copypasta
- Nếu muốn copy SSH command
- Logging: tracing hoặc log + env_logger
- Logging debug dev

# Cấu trúc file gợi ý

sshr/
├── Cargo.toml
├── src/
│ ├── main.rs
│ ├── app.rs # App state và logic chính
│ ├── ui.rs # Vẽ giao diện TUI
│ ├── ssh_service.rs # Chạy ssh, scp, build command
│ ├── config_manager.rs # Đọc / ghi danh sách host
│ ├── models.rs # Struct như SshHost, Group
│ ├── event.rs # Xử lý event
│ └── error.rs # Xử lý error
├── assets/
│ └── default_config.json

# UI mockup ý tưởng (ý tưởng TUI)

┌────────────────────────────── SSHr ──────────────────────────────┐
│ [1] dev-vm (dev@192.168.1.2:22) [Group: dev] │
│ [2] prod-db (admin@10.0.0.10:2222) [Group: production] │
│ [3] test-api (test@35.247.x.x:22) [Group: staging] │
├──────────────────────────────────────────────────────────────────┤
│ ↑↓ để chọn · [Enter] Kết nối · [a] Thêm · [e] Sửa · [d] Xoá │
└──────────────────────────────────────────────────────────────────┘

# Hotkey gợi ý

Phím : Hành động
↑ / ↓: Di chuyển trong danh sách
Enter: SSH vào host đang chọn
a: Thêm host mới
e: Sửa host đang chọn
d: Xoá host
q: Thoát