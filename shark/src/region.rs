#[non_exhaustive]
pub enum Region {
    Us,
    Eu,
}

impl Region {
    pub(crate) fn user_url(&self) -> &'static str {
        match *self {
            Self::Us => "https://user-field-39a9391a.aylanetworks.com",
            Self::Eu => "https://user-field-eu.aylanetworks.com",
        }
    }

    pub(crate) fn device_url(&self) -> &'static str {
        match *self {
            Self::Us => "https://ads-field-39a9391a.aylanetworks.com",
            Self::Eu => "https://ads-eu.aylanetworks.com",
        }
    }

    pub(crate) fn app_id(&self) -> &'static str {
        match *self {
            Self::Us => "Shark-Android-field-id",
            Self::Eu => "Shark-Android-EUField-Fw-id",
        }
    }

    pub(crate) fn app_secret(&self) -> &'static str {
        match *self {
            Self::Us => "Shark-Android-field-Wv43MbdXRM297HUHotqe6lU1n-w",
            Self::Eu => "Shark-Android-EUField-s-zTykblGJujGcSSTaJaeE4PESI",
        }
    }
}
