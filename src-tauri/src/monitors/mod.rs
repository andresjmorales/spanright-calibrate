pub mod models;
mod discovery;
mod edid;

pub use models::Monitor;

pub fn discover_all() -> Result<Vec<Monitor>, String> {
    let mut monitors = discovery::enumerate_monitors()?;

    if let Err(e) = discovery::populate_friendly_names(&mut monitors) {
        eprintln!("Warning: could not get friendly names: {e}");
    }

    match edid::read_all_edid() {
        Ok(edid_map) => {
            edid::apply_edid_to_monitors(&mut monitors, &edid_map);
        }
        Err(e) => {
            eprintln!("Warning: could not read EDID data: {e}");
        }
    }

    for m in &mut monitors {
        m.compute_derived();
    }

    Ok(monitors)
}
