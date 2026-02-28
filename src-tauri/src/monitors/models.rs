use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: usize,
    pub device_name: String,
    pub friendly_name: String,
    pub monitor_name: String,
    pub adapter_name: String,
    /// Hardware path from EnumDisplayDevices, e.g. "MONITOR\HPN3645\{guid}\0001"
    pub monitor_device_id: String,
    pub is_primary: bool,

    pub resolution_x: u32,
    pub resolution_y: u32,
    pub position_x: i32,
    pub position_y: i32,
    pub orientation: u32,

    pub physical_width_mm: Option<u32>,
    pub physical_height_mm: Option<u32>,

    pub physical_width_in: Option<f64>,
    pub physical_height_in: Option<f64>,
    pub diagonal_in: Option<f64>,
    pub ppi: Option<f64>,
}

impl Monitor {
    pub fn compute_derived(&mut self) {
        if let (Some(w_mm), Some(h_mm)) = (self.physical_width_mm, self.physical_height_mm) {
            if w_mm == 0 || h_mm == 0 {
                return;
            }
            let w_in = w_mm as f64 / 25.4;
            let h_in = h_mm as f64 / 25.4;
            self.physical_width_in = Some(w_in);
            self.physical_height_in = Some(h_in);
            self.diagonal_in = Some((w_in * w_in + h_in * h_in).sqrt());

            let diagonal_px =
                ((self.resolution_x as f64).powi(2) + (self.resolution_y as f64).powi(2)).sqrt();
            if let Some(diag) = self.diagonal_in {
                if diag > 0.0 {
                    self.ppi = Some(diagonal_px / diag);
                }
            }
        }
    }
}
