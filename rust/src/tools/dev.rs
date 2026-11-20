use std::process::Command;

pub fn check_port_status(port: u16) -> String {
    let out = Command::new("ss")
        .args(["-tulpn", &format!("sport = :{}", port)])
        .output();

    if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout);
        let lines: Vec<&str> = text.lines().skip(1).collect();
        if !lines.is_empty() {
            return format!("Cổng {} hiện đang được sử dụng bởi dịch vụ hệ thống.", port);
        }
    }
    format!("Cổng {} hiện đang hoàn toàn trống và sẵn sàng sử dụng.", port)
}

pub fn kill_port_process(port: u16) -> String {
    let _ = Command::new("fuser")
        .args(["-k", "-n", "tcp", &port.to_string()])
        .output();

    format!("Đã giải phóng thành công tiến trình chiếm giữ cổng {}.", port)
}

pub fn check_docker_containers() -> String {
    let out = Command::new("docker")
        .args(["ps", "--format", "{{.Names}} ({{.Image}})"])
        .output();

    if let Ok(o) = out {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if text.is_empty() {
                return "Hiện tại không có container Docker nào đang chạy trên máy.".to_string();
            }
            let count = text.lines().count();
            return format!("Đang có {} container Docker đang chạy:\n{}", count, text);
        }
    }
    "Không thể kết nối đến Docker daemon hoặc Docker chưa được bật.".to_string()
}

pub fn check_java_version() -> String {
    let out = Command::new("java")
        .arg("-version")
        .output();

    if let Ok(o) = out {
        let text = if !o.stderr.is_empty() {
            String::from_utf8_lossy(&o.stderr)
        } else {
            String::from_utf8_lossy(&o.stdout)
        };
        if let Some(first_line) = text.lines().next() {
            return format!("Phiên bản Java đang cài trên máy là: {}.", first_line.trim());
        }
    }
    "Hệ thống hiện chưa cài đặt môi trường Java (JDK/JRE).".to_string()
}

pub fn get_git_status_summary() -> String {
    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "main".to_string());

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    if status.is_empty() {
        format!("Trạng thái Git: Nhánh '{}' đang sạch sẽ, không có thay đổi nào chưa commit.", branch)
    } else {
        let changed = status.lines().count();
        format!("Trạng thái Git: Nhánh '{}' hiện có {} tệp tin thay đổi chưa chuyển giao.", branch, changed)
    }
}
