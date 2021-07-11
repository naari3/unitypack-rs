use nom::{
    bytes::complete::tag,
    character::complete::alpha1,
    multi::count,
    number::complete::{be_u32, be_u64, be_u8},
    IResult,
};

pub mod asset;
pub mod asset_bundle;

fn read_string_to_null(input: &[u8]) -> IResult<&[u8], String> {
    let (input, signature) = alpha1(input)?;
    let (input, _) = tag(b"\0")(input)?;
    Ok((input, std::str::from_utf8(signature).unwrap().to_string()))
}

enum FileType {
    AssetsFile,
    BundleFile,
    WebFile,
    ResourceFile,
}

fn check_file_type(input: &[u8]) -> IResult<&[u8], FileType> {
    let (input, signature) = read_string_to_null(input)?;
    let (input, file_type) = match signature.as_str() {
        "UnityWeb" | "UnityRaw" | "UnityArchive" | "UnityFS" => (input, FileType::BundleFile),
        "UnityWebData1.0" => (input, FileType::WebFile),
        _ => {
            // let magic = input[..2];
            // todo!("check magic for gzip");
            // let magic = input[20..26];
            // todo!("check magic for brotil");
            let (_input, is_serialized_file) = check_serialized_file(input)?;
            if is_serialized_file {
                (input, FileType::AssetsFile)
            } else {
                (input, FileType::ResourceFile)
            }
        }
    };
    Ok((input, file_type))
}

fn check_serialized_file(input: &[u8]) -> IResult<&[u8], bool> {
    let original_input = input.clone();

    if input.len() < 20 {
        return Ok((input, false));
    }

    let (input, mut _metadata_size) = be_u32(input)?;
    let (input, mut file_size) = be_u32(input)?;
    let (input, version) = be_u32(input)?;
    let (input, mut data_offset) = be_u32(input)?;
    let (input, _endianess) = be_u8(input)?;
    let (mut input, _reserved) = count(be_u8, 3)(input)?;
    if version >= 22 {
        if file_size < 48 {
            return Ok((original_input, false));
        }
        let result = be_u32(input)?;
        input = result.0;
        _metadata_size = result.1;
        let result = be_u64(input)?;
        input = result.0;
        file_size = result.1 as u32;
        let result = be_u64(input)?;
        input = result.0;
        data_offset = result.1 as u32;
    }
    let input = original_input;
    let real_file_size = original_input.len() as u32;
    if file_size != real_file_size {
        return Ok((input, false));
    };
    if data_offset > real_file_size {
        return Ok((input, false));
    }
    Ok((input, true))
}
