use std::{ops::Deref, sync::LazyLock};

use gettextrs::gettext;

use super::models::RxUiFeature;

pub static TWO_FACTOR_AUTH_VIEW_INSTRUCTIONS: LazyLock<String> = LazyLock::new(|| {
    gettext(
        "<b>Showing all 2FA codes in the database.</b><br/>View an entry's details using its action bar.",
    )
});

pub fn get_instructions(feature: &RxUiFeature) -> Option<String> {
    match feature {
        RxUiFeature::DisplayTwoFactorAuth => {
            Some(TWO_FACTOR_AUTH_VIEW_INSTRUCTIONS.to_owned())
        }
        _ => None,
    }
}
