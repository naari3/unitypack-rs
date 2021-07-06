use nom::{
    bytes::complete::{tag, take_until},
    character::complete::alpha1,
    number::streaming::be_u32,
    IResult,
};

#[derive(Debug)]
struct UnityAsset {
    signature: String,
    version: u32,
    unity_version: String,
    unity_revision: String,
}

fn read_unity_asset_header(input: &[u8]) -> IResult<&[u8], UnityAsset> {
    let (input, signature) = alpha1(input)?;
    let (input, _) = tag(b"\0")(input)?;
    let signature = std::str::from_utf8(signature).unwrap().to_string();

    let (input, version) = be_u32(input)?;

    let (input, unity_version) = take_until("\0")(input)?;
    let unity_version = std::str::from_utf8(unity_version).unwrap().to_string();
    let (input, _) = tag(b"\0")(input)?;

    let (input, unity_revision) = take_until("\0")(input)?;
    let unity_revision = std::str::from_utf8(unity_revision).unwrap().to_string();
    let (input, _) = tag(b"\0")(input)?;

    Ok((
        input,
        UnityAsset {
            signature,
            version,
            unity_version,
            unity_revision,
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::{io::Read, path::Path};

    use crate::read_unity_asset_header;

    fn read_file<P: AsRef<Path>>(file_path: P) -> Vec<u8> {
        let mut file = std::fs::File::open(file_path).expect("file open failed");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("file read failed");
        buf
    }

    #[test]
    fn test_read_unity_asset_header() {
        let file = read_file("./anm_chara_tear_animator");
        let unity_asset = read_unity_asset_header(&file).unwrap().1;
        println!("{:?}", unity_asset);
        assert_eq!("UnityFS", unity_asset.signature);
        assert_eq!(6, unity_asset.version);
        assert_eq!("5.x.x", unity_asset.unity_version);
        assert_eq!("2019.4.1f1", unity_asset.unity_revision);
    }
}
