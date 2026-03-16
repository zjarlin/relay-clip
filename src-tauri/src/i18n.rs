use crate::models::{AppLanguage, ClipboardPayloadKind};

pub fn payload_kind(language: AppLanguage, kind: &ClipboardPayloadKind) -> &'static str {
    match (language, kind) {
        (AppLanguage::ZhCn, ClipboardPayloadKind::Text) => "文本",
        (AppLanguage::ZhCn, ClipboardPayloadKind::Image) => "图片",
        (_, ClipboardPayloadKind::Text) => "text",
        (_, ClipboardPayloadKind::Image) => "image",
    }
}

pub fn advertising(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "正在局域网中广播 RelayClip 服务".to_string(),
        AppLanguage::En => "Advertising RelayClip on the local network".to_string(),
    }
}

pub fn paused(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "剪贴板同步已暂停".to_string(),
        AppLanguage::En => "Clipboard sync is paused".to_string(),
    }
}

pub fn no_paired_devices(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "当前还没有已配对设备".to_string(),
        AppLanguage::En => "No paired devices selected yet".to_string(),
    }
}

pub fn sending(language: AppLanguage, kind: &ClipboardPayloadKind, device_count: usize) -> String {
    match language {
        AppLanguage::ZhCn => {
            format!(
                "正在向 {device_count} 台设备发送{}",
                payload_kind(language, kind)
            )
        }
        AppLanguage::En => format!(
            "Sending {} clipboard to {device_count} paired device(s)",
            payload_kind(language, kind)
        ),
    }
}

pub fn relayed(language: AppLanguage, kind: &ClipboardPayloadKind) -> String {
    match language {
        AppLanguage::ZhCn => format!("{}已接力发送", payload_kind(language, kind)),
        AppLanguage::En => format!("{} relayed", payload_kind(language, kind)),
    }
}

pub fn relay_failed(language: AppLanguage, error: &str) -> String {
    match language {
        AppLanguage::ZhCn => format!("剪贴板接力失败：{error}"),
        AppLanguage::En => format!("Failed to relay clipboard: {error}"),
    }
}

pub fn received(language: AppLanguage, kind: &ClipboardPayloadKind) -> String {
    match language {
        AppLanguage::ZhCn => format!("已接收{}", payload_kind(language, kind)),
        AppLanguage::En => format!("Received {}", payload_kind(language, kind)),
    }
}

pub fn discovery_disabled(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "已关闭局域网发现".to_string(),
        AppLanguage::En => "LAN discovery is disabled".to_string(),
    }
}

pub fn paired_devices_ready(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::ZhCn => format!("已有 {count} 台已配对设备在线，可直接接力"),
        AppLanguage::En => format!("{count} paired device(s) online and ready"),
    }
}

pub fn paired_devices_offline(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "已配对设备当前离线".to_string(),
        AppLanguage::En => "Paired devices are currently offline".to_string(),
    }
}

pub fn available_peers(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::ZhCn => format!("已发现 {count} 台可配对设备"),
        AppLanguage::En => format!("Found {count} nearby device(s) ready to pair"),
    }
}

pub fn looking_for_peers(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "正在局域网中查找 RelayClip 设备".to_string(),
        AppLanguage::En => "Looking for RelayClip peers on the LAN".to_string(),
    }
}

pub fn sync_status_unavailable(language: AppLanguage) -> String {
    match language {
        AppLanguage::ZhCn => "同步状态不可用".to_string(),
        AppLanguage::En => "Sync status unavailable".to_string(),
    }
}

pub fn route_lookup_failed(language: AppLanguage, error: &str) -> String {
    match language {
        AppLanguage::ZhCn => format!("查找剪贴板路由失败：{error}"),
        AppLanguage::En => format!("Clipboard route lookup failed: {error}"),
    }
}

pub fn active_label(language: AppLanguage, active_name: &str) -> String {
    match language {
        AppLanguage::ZhCn => format!("已配对：{active_name}"),
        AppLanguage::En => format!("Paired: {active_name}"),
    }
}

pub fn no_active_device_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "暂无已配对设备",
        AppLanguage::En => "No paired devices",
    }
}

pub fn tray_paired_devices(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "已配对设备",
        AppLanguage::En => "Paired Devices",
    }
}

pub fn tray_nearby_devices(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "附近设备",
        AppLanguage::En => "Nearby Devices",
    }
}

pub fn tray_waiting_for_devices(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "等待设备出现",
        AppLanguage::En => "Waiting for devices",
    }
}

pub fn tray_pause_sync(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "暂停同步",
        AppLanguage::En => "Pause Sync",
    }
}

pub fn tray_resume_sync(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "恢复同步",
        AppLanguage::En => "Resume Sync",
    }
}

pub fn tray_open_settings(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "打开设置",
        AppLanguage::En => "Open Settings",
    }
}

pub fn tray_quit(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::ZhCn => "退出",
        AppLanguage::En => "Quit",
    }
}
