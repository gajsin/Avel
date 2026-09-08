use super::model::BrightnessDisplay;

pub fn generate_diagnostics_report(displays: &[BrightnessDisplay], app_version: &str) -> String {
    let mut report = String::new();
    report.push_str(&format!(
        "Avel Brightness Diagnostics Report (v{})\n",
        app_version
    ));
    report.push_str(&format!(
        "Generated at: {}\n",
        chrono::Local::now().to_rfc3339()
    ));
    report.push_str("----------------------------------------\n");
    report.push_str(&format!("Detected Displays: {}\n\n", displays.len()));

    for (i, d) in displays.iter().enumerate() {
        report.push_str(&format!("[Display #{}]\n", i + 1));
        report.push_str(&format!("  ID: {}\n", d.id));
        report.push_str(&format!("  Name: {}\n", d.name));
        report.push_str(&format!("  Primary: {}\n", d.is_primary));
        report.push_str(&format!(
            "  Connection: {}\n",
            d.connection.as_deref().unwrap_or("Unknown")
        ));
        report.push_str(&format!(
            "  Backend: {}\n",
            d.backend
                .map(|b| b.as_str().to_string())
                .unwrap_or_else(|| "None".to_string())
        ));
        report.push_str(&format!("  Probe State: {:?}\n", d.probe_state));
        report.push_str(&format!("  Verification: {}\n", d.verification.as_str()));
        report.push_str(&format!(
            "  Current Brightness: {}%\n",
            d.current_percent
                .map(|p| p.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
        report.push_str(&format!(
            "  Raw Range: [{} - {}]\n",
            d.raw_min
                .map(|m| m.to_string())
                .unwrap_or_else(|| "—".to_string()),
            d.raw_max
                .map(|m| m.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
        if let Some(ref lvls) = d.available_levels {
            report.push_str(&format!("  Supported Discrete Levels: {:?}\n", lvls));
        }
        if let Some(ref err) = d.error {
            report.push_str(&format!(
                "  Last Error: {} (op: {}, code: {:?}, retryable: {})\n",
                err.code, err.operation, err.native_code, err.retryable
            ));
        }
        report.push('\n');
    }

    report.push_str("----------------------------------------\n");
    report.push_str("End of Report\n");
    report
}
