//! The embedded font files. The reader, the UI and the text covers use the
//! same bytes, so each file is in the exe one time.

pub struct Family {
    pub regular: &'static [u8],
    pub bold: &'static [u8],
    pub italic: &'static [u8],
    pub bold_italic: &'static [u8],
}

impl Family {
    pub fn style(&self, bold: bool, italic: bool) -> &'static [u8] {
        match (bold, italic) {
            (false, false) => self.regular,
            (true, false) => self.bold,
            (false, true) => self.italic,
            (true, true) => self.bold_italic,
        }
    }

    pub fn files(&self) -> [&'static [u8]; 4] {
        [self.regular, self.bold, self.italic, self.bold_italic]
    }
}

macro_rules! family {
    ($name:literal) => {
        Family {
            regular: include_bytes!(concat!("../assets/fonts/", $name, "-Regular.ttf")),
            bold: include_bytes!(concat!("../assets/fonts/", $name, "-Bold.ttf")),
            italic: include_bytes!(concat!("../assets/fonts/", $name, "-Italic.ttf")),
            bold_italic: include_bytes!(concat!("../assets/fonts/", $name, "-BoldItalic.ttf")),
        }
    };
}

pub static LITERATA: Family = family!("Literata");
pub static INTER: Family = family!("Inter");
pub static NEWSREADER: Family = family!("Newsreader");
pub static SOURCE_SERIF: Family = family!("SourceSerif4");
pub static ALEGREYA: Family = family!("Alegreya");
pub static GARAMOND: Family = family!("EBGaramond");
pub static PLEX_MONO: Family = family!("IBMPlexMono");
pub static PLEX_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf");
pub static DOT_GOTHIC: &[u8] = include_bytes!("../assets/fonts/DotGothic16-Regular.ttf");

/// The families of the reader.
pub static READER: [&Family; 7] = [
    &LITERATA,
    &INTER,
    &NEWSREADER,
    &SOURCE_SERIF,
    &ALEGREYA,
    &GARAMOND,
    &PLEX_MONO,
];
