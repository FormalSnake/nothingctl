//! Device pictures from ear-web (AGPL-3.0), fetched at run time from a pinned commit and cached
//! by the image pipeline, so none of its art ships in this repo.

const EAR_WEB: &str =
    "https://raw.githubusercontent.com/radiance-project/ear-web/d076e1371572fbf0a14b441f9b3cafbf9a237e04/res/assets";

/// Fast Pair model ids to ear-web's picture sets (res/js/control.js
/// `FASTPAIR_MODELS`), where the set names the colourway. Ear (a) and CMF
/// Buds Pro 2 carry two picture families under one base.
#[rustfmt::skip]
const ART: [(&str, &str); 74] = [
    ("624011", "b181_black"), ("31D53D", "b181_white"),
    ("1016DD", "b157_white"),
    ("ACC520", "b155_white"), ("DEE8C0", "b155_black"),
    ("FEB1C7", "b171_white"), ("A20444", "b171_black"),
    ("5E3FBC", "b162_white"), ("03464E", "b162_black"), ("8B6380", "b162_yellow"),
    ("C34F3B", "b183_black"), ("404D6D", "b183_white"), ("839E9A", "b183_yellow"),
    ("FC3AAF", "b174_white"), ("CC3444", "b174_blue"),
    ("5F8F82", "b163_orange"), ("2EB1CA", "b163_white"), ("ADD2C4", "b163_black"),
    ("4DFC4A", "b164_orange"), ("26C190", "b164_white"), ("AE35FD", "b164_black"),
    ("D35E18", "b168_orange"), ("ACCE54", "b168_white"), ("150A27", "b168_black"),
    ("F29566", "b172_black"), ("CA36A6", "b172_white"), ("A7B220", "b172_orange"), ("2B353E", "b172_blue"),
    ("2F45F5", "b187_black"), ("E1BE45", "b187_white"), ("1253C0", "b187_orange"), ("0F1A4F", "b187_blue"),
    ("F51D3D", "b187_black"), ("A544CE", "b187_white"), ("9EAC04", "b187_orange"), ("1D07D4", "b187_blue"),
    ("53BFE6", "b187_green"), ("072714", "b187_pink"),
    ("19EF24", "b179_black"), ("FF2AB0", "b179_green"), ("D9AB5D", "b179_orange"),
    ("3B1320", "b179_black"), ("359A7D", "b179_green"), ("00FDED", "b179_orange"),
    ("5C587F", "b184_blue"), ("4AEB6E", "b184_white"),
    ("70F8E3", "b185_black"), ("ED5412", "b185_white"), ("509CAE", "b185_orange"),
    ("C19ECD", "b170_black"), ("2D6FDA", "b170_grey"),
    ("7D46E5", "b173_black"), ("C1EBFD", "b173_white"),
    ("563DA5", "b175_green"), ("73C9EB", "b175_white"), ("1EFB39", "b175_black"),
    ("BFD53B", "b186_black"), ("DE8953", "b186_white"), ("98D02B", "b186_pink"),
    ("810478", "b198_white"), ("97EF75", "b198_yellow"), ("6F6C71", "b198_black"),
    ("79B3A9", "b198_yellow"), ("A292C6", "b198_pink"),
    ("148887", "b190_pink"), ("7B2328", "b190_white"), ("E9C3BE", "b190_black"), ("DB45D3", "b190_yellow"),
    ("8F28ED", "b189_blue"), ("E6673A", "b189_orange"), ("DCD2CB", "b189_white"), ("DA1280", "b189_black"),
    ("CDFDEC", "b192_black"), ("086485", "b192_silver"),
];

/// The picture of both buds or the headphone: the model id's colourway, else the base's black
/// (its first set where ear-web has no black).
pub fn url(base: &str, id: &str) -> Option<String> {
    let set = ART.iter().find(|(i, _)| i.eq_ignore_ascii_case(id)).map(|(_, set)| *set).or_else(|| {
        let base = base.to_ascii_lowercase();
        let mut sets = ART.iter().map(|(_, set)| *set).filter(|set| set.split('_').next() == Some(base.as_str()));
        let first = sets.clone().next();
        sets.find(|set| set.ends_with("_black")).or(first)
    })?;
    Some(format!("{EAR_WEB}/{set}_duo.webp"))
}
