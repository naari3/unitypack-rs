use nom::{
    bytes::complete::{tag, take, take_until},
    character::complete::alpha1,
    multi::count,
    number::{
        complete::{be_i32, be_i64, be_u16},
        streaming::be_u32,
    },
    IResult,
};

#[derive(Debug)]
struct UnityAsset {
    header: UnityAssetHeader,
    container_header: UnityContainerHeader,
    storage_block: Vec<UnityStorageBlock>,
    directory_info: Vec<UnityNode>,
}

#[derive(Debug)]
struct UnityAssetHeader {
    signature: String,
    version: u32,
    unity_version: String,
    unity_revision: String,
}

#[derive(Debug, Default)]
struct UnityContainerHeader {
    size: i64,
    compressed_blocks_info_size: u32,
    uncompressed_blocks_info_size: u32,
    flags: u32,
}

#[derive(Debug, Default)]
struct UnityStorageBlock {
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
}

#[derive(Debug, Default)]
struct UnityNode {
    offset: i64,
    size: i64,
    flags: u32,
    path: String,
}

fn read_unity_asset(input: &[u8]) -> IResult<&[u8], UnityAsset> {
    let (input, unity_asset_header) = read_unity_asset_header(input)?;
    let (input, unity_container_header) = read_unity_container_header(input)?;
    let (input, ((unity_asset_header, unity_container_header), (storage_block, directory_info))) =
        read_blocks_info_and_directory(input, unity_asset_header, unity_container_header)?;

    Ok((
        input,
        UnityAsset {
            header: unity_asset_header,
            container_header: unity_container_header,
            storage_block,
            directory_info,
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
        UnityContainerHeader {
            size,
            compressed_blocks_info_size,
            uncompressed_blocks_info_size,
            flags,
        },
    ))
}

fn read_blocks_info_and_directory(
    input: &[u8],
    header: UnityAssetHeader,
    container_header: UnityContainerHeader,
) -> IResult<
    &[u8],
    (
        (UnityAssetHeader, UnityContainerHeader),
        (Vec<UnityStorageBlock>, Vec<UnityNode>),
    ),
> {
    if header.version >= 7 {
        // 16bytes align
        todo!()
    }

    let (input, compressed_blocks_info_bytes) = if (container_header.flags & 0x80) != 0 {
        // kArchiveBlocksInfoAtTheEnd
        let bytes = &input[(input.len() - container_header.compressed_blocks_info_size as usize)..];
        (input, bytes)
    } else {
        // kArchiveBlocksAndDirectoryInfoCombined
        take(container_header.compressed_blocks_info_size)(input)?
    };
    let block_info = match container_header.flags & 0x3F {
        //kArchiveCompressionTypeMask
        1 => todo!(), // LZMA
        2 | 3 => {
            let decoded = lz4_flex::block::decompress(
                compressed_blocks_info_bytes,
                container_header.uncompressed_blocks_info_size as usize,
            )
            .unwrap();
            decoded
        } // LZ4, LZ4HC
        _ => (compressed_blocks_info_bytes.to_owned().to_vec()), // None
    };
    // let block_info = block_info.as_slice();
    let (_block_info, (storage_blocks, nodes)) = read_block_infos(&block_info).unwrap();

    Ok((input, ((header, container_header), (storage_blocks, nodes))))
}

fn read_block_infos(block_info: &[u8]) -> IResult<&[u8], (Vec<UnityStorageBlock>, Vec<UnityNode>)> {
    let (block_info, _data_hash) = take(16usize)(block_info)?;

    let (block_info, blocks_info_count) = be_i32(block_info)?;
    let (block_info, storage_blocks) =
        count(read_storage_block, blocks_info_count as usize)(block_info)?;

    let (block_info, nodes_count) = be_i32(block_info)?;
    let (block_info, nodes) = count(read_node, nodes_count as usize)(block_info)?;

    Ok((block_info, (storage_blocks, nodes)))
}

fn read_storage_block(input: &[u8]) -> IResult<&[u8], UnityStorageBlock> {
    let (input, uncompressed_size) = be_u32(input)?;
    let (input, compressed_size) = be_u32(input)?;
    let (input, flags) = be_u16(input)?;

    Ok((
        input,
        UnityStorageBlock {
            compressed_size,
            uncompressed_size,
            flags,
        },
    ))
}

fn read_node(input: &[u8]) -> IResult<&[u8], UnityNode> {
    let (input, offset) = be_i64(input)?;
    let (input, size) = be_i64(input)?;
    let (input, flags) = be_u32(input)?;
    let (input, path) = take_until("\0")(input)?;
    let path = std::str::from_utf8(path).unwrap().to_string();
    let (input, _) = tag(b"\0")(input)?;

    Ok((
        input,
        UnityNode {
            offset,
            size,
            flags,
            path,
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::{io::Read, path::Path};

    use crate::read_unity_asset;

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
        assert_eq!(9185, unity_asset.container_header.size);
        assert_eq!(65, unity_asset.container_header.compressed_blocks_info_size);
        assert_eq!(
            91,
            unity_asset.container_header.uncompressed_blocks_info_size
        );
        assert_eq!(67, unity_asset.container_header.flags);
        assert_eq!(1, unity_asset.directory_info.len());
        assert_eq!(0, unity_asset.directory_info[0].offset);
        assert_eq!(34932, unity_asset.directory_info[0].size);
        assert_eq!(4, unity_asset.directory_info[0].flags);
        assert_eq!(
            "CAB-f946f47e8f8bb3ec3f2f2259084955c0",
            unity_asset.directory_info[0].path
        );
    }
}
