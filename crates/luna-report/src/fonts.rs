//! The report's typeface, IBM Plex Sans (SIL OFL 1.1, see Assets/fonts/OFL.txt).
//! Embedded so the PDF and the HTML look the same on every machine.

pub(crate) struct Face {
    pub family: &'static str,
    pub weight: u16,
    pub data: &'static [u8],
}

pub(crate) const FACES: [Face; 6] = [
    Face {
        family: "IBM Plex Sans",
        weight: 400,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSans-Regular.ttf"),
    },
    Face {
        family: "IBM Plex Sans",
        weight: 500,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSans-Medium.ttf"),
    },
    Face {
        family: "IBM Plex Sans",
        weight: 600,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSans-SemiBold.ttf"),
    },
    Face {
        family: "IBM Plex Sans",
        weight: 700,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSans-Bold.ttf"),
    },
    Face {
        family: "IBM Plex Sans Condensed",
        weight: 500,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSansCondensed-Medium.ttf"),
    },
    Face {
        family: "IBM Plex Sans Condensed",
        weight: 600,
        data: include_bytes!("../../../Assets/fonts/IBMPlexSansCondensed-SemiBold.ttf"),
    },
];
