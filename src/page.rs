#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaperSize {
    #[default]
    Letter,
    Oficio,
    Legal,
    A4,
    A5,
}

impl PaperSize {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "letter" | "carta" => Self::Letter,
            "oficio" => Self::Oficio,
            "legal" => Self::Legal,
            "a4" => Self::A4,
            "a5" => Self::A5,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Letter => "letter",
            Self::Oficio => "oficio",
            Self::Legal => "legal",
            Self::A4 => "a4",
            Self::A5 => "a5",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Letter => "CARTA",
            Self::Oficio => "OFICIO",
            Self::Legal => "LEGAL",
            Self::A4 => "A4",
            Self::A5 => "A5",
        }
    }

    /// Physical dimensions in tenths of a millimetre, portrait orientation.
    pub fn dimensions_tenth_mm(self) -> (u16, u16) {
        match self {
            Self::Letter => (2_159, 2_794),
            Self::Oficio => (2_159, 3_302),
            Self::Legal => (2_159, 3_556),
            Self::A4 => (2_100, 2_970),
            Self::A5 => (1_480, 2_100),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageOrientation {
    #[default]
    Portrait,
    Landscape,
}

impl PageOrientation {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "portrait" | "vertical" => Self::Portrait,
            "landscape" | "horizontal" => Self::Landscape,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Portrait => "VERTICAL",
            Self::Landscape => "HORIZONTAL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageProfile {
    pub paper: PaperSize,
    pub orientation: PageOrientation,
    pub margin_top_mm: u16,
    pub margin_right_mm: u16,
    pub margin_bottom_mm: u16,
    pub margin_left_mm: u16,
}

impl Default for PageProfile {
    fn default() -> Self {
        Self {
            paper: PaperSize::Letter,
            orientation: PageOrientation::Portrait,
            margin_top_mm: 25,
            margin_right_mm: 25,
            margin_bottom_mm: 25,
            margin_left_mm: 25,
        }
    }
}

impl PageProfile {
    pub fn page_size_tenth_mm(self) -> (u16, u16) {
        let (width, height) = self.paper.dimensions_tenth_mm();
        match self.orientation {
            PageOrientation::Portrait => (width, height),
            PageOrientation::Landscape => (height, width),
        }
    }

    pub fn content_size_tenth_mm(self) -> (u16, u16) {
        let (width, height) = self.page_size_tenth_mm();
        let horizontal = self
            .margin_left_mm
            .saturating_add(self.margin_right_mm)
            .saturating_mul(10);
        let vertical = self
            .margin_top_mm
            .saturating_add(self.margin_bottom_mm)
            .saturating_mul(10);
        (
            width.saturating_sub(horizontal).max(200),
            height.saturating_sub(vertical).max(200),
        )
    }

    /// Logical hard-wrap width derived from the printable page width.
    /// Letter portrait with 25 mm side margins is the reference profile: 88 columns.
    pub fn text_columns(self) -> usize {
        const LETTER_PRINTABLE_TENTH_MM: u32 = 1_659;
        const LETTER_REFERENCE_COLUMNS: u32 = 88;
        let printable = u32::from(self.content_size_tenth_mm().0);
        let columns = (printable * LETTER_REFERENCE_COLUMNS + LETTER_PRINTABLE_TENTH_MM / 2)
            / LETTER_PRINTABLE_TENTH_MM;
        columns.clamp(24, 180) as usize
    }

    pub fn cycle_margin(value: u16) -> u16 {
        const VALUES: [u16; 7] = [10, 15, 20, 25, 30, 35, 40];
        VALUES
            .iter()
            .position(|candidate| *candidate == value)
            .map(|index| VALUES[(index + 1) % VALUES.len()])
            .unwrap_or(25)
    }

    pub fn docx_page_twips(self) -> (u32, u32) {
        let (width, height) = self.page_size_tenth_mm();
        (tenth_mm_to_twips(width), tenth_mm_to_twips(height))
    }

    pub fn docx_margin_twips(self) -> (u32, u32, u32, u32) {
        (
            mm_to_twips(self.margin_top_mm),
            mm_to_twips(self.margin_right_mm),
            mm_to_twips(self.margin_bottom_mm),
            mm_to_twips(self.margin_left_mm),
        )
    }

    pub fn pdf_page_points(self) -> (f32, f32) {
        let (width, height) = self.page_size_tenth_mm();
        (tenth_mm_to_points(width), tenth_mm_to_points(height))
    }

    pub fn pdf_margins_points(self) -> (f32, f32, f32, f32) {
        (
            mm_to_points(self.margin_top_mm),
            mm_to_points(self.margin_right_mm),
            mm_to_points(self.margin_bottom_mm),
            mm_to_points(self.margin_left_mm),
        )
    }
}

fn tenth_mm_to_twips(value: u16) -> u32 {
    // 1 inch = 25.4 mm = 1440 twips. value is tenths of a millimetre.
    (u32::from(value) * 720 + 63) / 127
}

fn mm_to_twips(value: u16) -> u32 {
    (u32::from(value) * 7_200 + 63) / 127
}

fn tenth_mm_to_points(value: u16) -> f32 {
    f32::from(value) * 72.0 / 254.0
}

fn mm_to_points(value: u16) -> f32 {
    f32::from(value) * 72.0 / 25.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_sizes_keep_physical_dimensions() {
        assert_eq!(PaperSize::Letter.dimensions_tenth_mm(), (2159, 2794));
        assert_eq!(PaperSize::Oficio.dimensions_tenth_mm(), (2159, 3302));
        assert_eq!(PaperSize::Legal.dimensions_tenth_mm(), (2159, 3556));
        assert_eq!(PaperSize::A4.dimensions_tenth_mm(), (2100, 2970));
        assert_eq!(PaperSize::A5.dimensions_tenth_mm(), (1480, 2100));
    }

    #[test]
    fn landscape_swaps_dimensions_without_touching_margins() {
        let page = PageProfile {
            paper: PaperSize::A4,
            orientation: PageOrientation::Landscape,
            ..PageProfile::default()
        };
        assert_eq!(page.page_size_tenth_mm(), (2970, 2100));
        assert_eq!(page.content_size_tenth_mm(), (2470, 1600));
    }

    #[test]
    fn letter_docx_dimensions_are_standard_twips() {
        let page = PageProfile::default();
        let (width, height) = page.docx_page_twips();
        assert!((width as i32 - 12_240).abs() <= 2);
        assert!((height as i32 - 15_840).abs() <= 2);
    }
}
