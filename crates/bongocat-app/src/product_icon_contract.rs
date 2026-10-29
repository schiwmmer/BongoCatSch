pub(crate) fn validate_icns(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() < 8 || &bytes[..4] != b"icns" {
        return Err("missing icns header");
    }
    let declared_length =
        u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| "missing icns length")?) as usize;
    if declared_length != bytes.len() {
        return Err("icns length does not match file size");
    }
    Ok(())
}

pub(crate) fn validate_png(bytes: &[u8]) -> Result<(), &'static str> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[..8] != PNG_SIGNATURE {
        return Err("missing png header");
    }

    if bytes.len() < 33 {
        return Err("png is missing its IHDR chunk");
    }
    let ihdr_length = u32::from_be_bytes(
        bytes[8..12]
            .try_into()
            .map_err(|_| "missing png IHDR length")?,
    );
    if ihdr_length != 13 || &bytes[12..16] != b"IHDR" {
        return Err("png is missing its IHDR chunk");
    }

    let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| "missing png width")?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| "missing png height")?);
    if width != 256 || height != 256 {
        return Err("png must be 256 by 256 pixels");
    }
    if bytes[24] != 8 {
        return Err("png must use 8-bit channels");
    }
    if bytes[25] != 6 {
        return Err("png must use RGBA color");
    }
    if bytes[28] != 0 {
        return Err("png must not be interlaced");
    }
    if bytes[26] != 0 || bytes[27] != 0 {
        return Err("png must use deflate compression and filtering");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_icns, validate_png};

    const PRODUCT_ICNS: &[u8] = include_bytes!("../../../resources/icons/logo-macos.icns");
    const MACOS_STATUS_PNG: &[u8] = include_bytes!("../../../resources/icons/tray-macos.png");

    #[test]
    fn native_product_icons_have_valid_containers() {
        validate_icns(PRODUCT_ICNS).expect("valid Native macOS icon");
        validate_png(MACOS_STATUS_PNG).expect("valid Native macOS status icon");
    }

    #[test]
    fn malformed_product_icons_are_rejected() {
        assert_eq!(validate_icns(b"icon"), Err("missing icns header"));
        assert_eq!(validate_png(b"png"), Err("missing png header"));
    }
}
