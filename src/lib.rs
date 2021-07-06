use nom::{
    bytes::complete::{tag, take_until},
    character::complete::alpha1,
    number::{complete::be_i64, streaming::be_u32},
    IResult,
};

#[derive(Debug)]
struct UnityAsset {
    header: UnityAssetHeader,
    container_header: UnityContainerHeader,
}

#[derive(Debug)]
struct UnityAssetHeader {
    signature: String,
    version: u32,
    unity_version: String,
    unity_revision: String,
}

#[derive(Debug)]
enum UnityContainerHeader {
    UnityFS {
        size: i64,
        compressed_blocks_info_size: u32,
        uncompressed_blocks_info_size: u32,
        flags: u32,
    },
}

fn read_unity_asset(input: &[u8]) -> IResult<&[u8], UnityAsset> {
    let (input, unity_asset_header) = read_unity_asset_header(input)?;
    let (input, unity_container_header) = read_unity_container_header(input)?;
    Ok((
        input,
        UnityAsset {
            header: unity_asset_header,
            container_header: unity_container_header,
        },
    ))
}

fn read_unity_asset_header(input: &[u8]) -> IResult<&[u8], UnityAssetHeader> {
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
        UnityAssetHeader {
            signature,
            version,
            unity_version,
            unity_revision,
        },
    ))
}

fn read_unity_container_header(input: &[u8]) -> IResult<&[u8], UnityContainerHeader> {
    let (input, size) = be_i64(input)?;
    let (input, compressed_blocks_info_size) = be_u32(input)?;
    let (input, uncompressed_blocks_info_size) = be_u32(input)?;
    let (input, flags) = be_u32(input)?;

    Ok((
        input,
        UnityContainerHeader::UnityFS {
            size,
            compressed_blocks_info_size,
            uncompressed_blocks_info_size,
            flags,
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::{io::Read, path::Path};

    use crate::{read_unity_asset, UnityContainerHeader};

    fn read_file<P: AsRef<Path>>(file_path: P) -> Vec<u8> {
        let mut file = std::fs::File::open(file_path).expect("file open failed");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("file read failed");
        buf
    }

    #[test]
    fn test_read_unity_asset_header() {
        let file = read_file("./anm_chara_tear_animator");
        let unity_asset = read_unity_asset(&file).unwrap().1;
        println!("{:?}", unity_asset);
        assert_eq!("UnityFS", unity_asset.header.signature);
        assert_eq!(6, unity_asset.header.version);
        assert_eq!("5.x.x", unity_asset.header.unity_version);
        assert_eq!("2019.4.1f1", unity_asset.header.unity_revision);
        assert!(matches!(
            unity_asset.container_header,
            UnityContainerHeader::UnityFS {
                size: 9185,
                compressed_blocks_info_size: 65,
                uncompressed_blocks_info_size: 91,
                flags: 67
            }
        ));
    }
}
