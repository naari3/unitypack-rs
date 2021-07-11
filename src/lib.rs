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
pub struct UnityAssetBundle {
    pub header: UnityAssetBundleHeader,
    pub container_header: UnityContainerHeader,
    pub storage_blocks: Vec<UnityStorageBlock>,
    pub directory_info: Vec<UnityNode>,
    pub stream_files: Vec<UnityStreamFile>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityAssetBundleHeader {
    pub signature: String,
    pub version: u32,
    pub unity_version: String,
    pub unity_revision: String,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityContainerHeader {
    pub size: i64,
    pub compressed_blocks_info_size: u32,
    pub uncompressed_blocks_info_size: u32,
    pub flags: u32,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityStorageBlock {
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub flags: u16,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityNode {
    pub offset: i64,
    pub size: i64,
    pub flags: u32,
    pub path: String,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnityStreamFile {
    pub path: String,
    pub file_name: String,
    pub body: Vec<u8>,
}

pub fn read_unity_asset_bundle(input: &[u8]) -> IResult<&[u8], UnityAssetBundle> {
    let (input, unity_asset_bundle_header) = read_unity_asset_bundle_header(input)?;
    let (input, unity_container_header) = read_unity_container_header(input)?;
    let (
        input,
        ((unity_asset_bundle_header, unity_container_header), (storage_blocks, directory_info)),
    ) = read_blocks_info_and_directory(input, unity_asset_bundle_header, unity_container_header)?;
    let (input, (stream_files, storage_blocks, directory_info)) =
        read_files(input, storage_blocks, directory_info)?;

    Ok((
        input,
        UnityAssetBundle {
            header: unity_asset_bundle_header,
            container_header: unity_container_header,
            storage_blocks,
            directory_info,
            stream_files,
        },
    ))
}

fn read_unity_asset_bundle_header(input: &[u8]) -> IResult<&[u8], UnityAssetBundleHeader> {
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
        UnityAssetBundleHeader {
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
    header: UnityAssetBundleHeader,
    container_header: UnityContainerHeader,
) -> IResult<
    &[u8],
    (
        (UnityAssetBundleHeader, UnityContainerHeader),
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
    let block_info = decompress(
        compressed_blocks_info_bytes,
        container_header.uncompressed_blocks_info_size as usize,
        container_header.flags,
    );
    // let block_info = block_info.as_slice();
    let (_block_info, (storage_blocks, nodes)) = read_block_infos(&block_info).unwrap();

    Ok((input, ((header, container_header), (storage_blocks, nodes))))
}

fn decompress(compressed_bytes: &[u8], uncompressed_size: usize, flags: u32) -> Vec<u8> {
    match flags & 0x3F {
        //kArchiveCompressionTypeMask
        1 => todo!(), // LZMA
        2 | 3 => {
            let decoded = lz4_flex::block::decompress(compressed_bytes, uncompressed_size).unwrap();
            decoded
        } // LZ4, LZ4HC
        _ => (compressed_bytes.to_owned().to_vec()), // None
    }
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

fn read_files(
    input: &[u8],
    storage_blocks: Vec<UnityStorageBlock>,
    directory_info: Vec<UnityNode>,
) -> IResult<&[u8], (Vec<UnityStreamFile>, Vec<UnityStorageBlock>, Vec<UnityNode>)> {
    let (input, (decompressed, storage_blocks)) =
        decompress_by_storage_blocks(input, storage_blocks)?;
    let (_decompressed, (stream_files, directory_info)) =
        read_stream_files(&decompressed, directory_info).unwrap();
    Ok((input, (stream_files, storage_blocks, directory_info)))
}

fn decompress_by_storage_blocks(
    input: &[u8],
    storage_blocks: Vec<UnityStorageBlock>,
) -> IResult<&[u8], (Vec<u8>, Vec<UnityStorageBlock>)> {
    let mut input = input;
    let mut decompresseds = Vec::with_capacity(storage_blocks.len());
    let mut dec = vec![];
    for sb in storage_blocks.iter() {
        let compressed_bytes: &[u8];
        let result = take(sb.compressed_size)(input)?;
        input = result.0;
        compressed_bytes = result.1;
        let decompressed = decompress(
            compressed_bytes,
            sb.uncompressed_size as usize,
            sb.flags as u32,
        );
        decompresseds.push(decompressed);
    }
    for mut d in decompresseds {
        dec.append(&mut d);
    }
    Ok((input, (dec, storage_blocks)))
}

fn read_stream_files(
    input: &[u8],
    directory_info: Vec<UnityNode>,
) -> IResult<&[u8], (Vec<UnityStreamFile>, Vec<UnityNode>)> {
    let mut input = input;
    let mut stream_files = vec![];
    for di in directory_info.iter() {
        let body: &[u8];
        let result = take(di.size as usize)(input)?;
        input = result.0;
        body = result.1;
        let sf = UnityStreamFile {
            path: di.path.clone(),
            file_name: di.path.clone(),
            body: body.to_vec(),
        };
        stream_files.push(sf);
    }

    Ok((input, (stream_files, directory_info)))
}

#[cfg(test)]
mod tests {
    use std::{io::Read, path::Path};

    use crate::read_unity_asset_bundle;

    fn read_file<P: AsRef<Path>>(file_path: P) -> Vec<u8> {
        let mut file = std::fs::File::open(file_path).expect("file open failed");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("file read failed");
        buf
    }

    #[test]
    fn test_read_unity_asset_bundle_header() {
        let file = read_file("./item_icon_00000");
        let unity_asset_bundle = read_unity_asset_bundle(&file).unwrap().1;
        assert_eq!("UnityFS", unity_asset_bundle.header.signature);
        assert_eq!(6, unity_asset_bundle.header.version);
        assert_eq!("5.x.x", unity_asset_bundle.header.unity_version);
        assert_eq!("2019.4.1f1", unity_asset_bundle.header.unity_revision);
        assert_eq!(4465, unity_asset_bundle.container_header.size);
        assert_eq!(
            85,
            unity_asset_bundle
                .container_header
                .compressed_blocks_info_size
        );
        assert_eq!(
            153,
            unity_asset_bundle
                .container_header
                .uncompressed_blocks_info_size
        );
        assert_eq!(67, unity_asset_bundle.container_header.flags);
        assert_eq!(2, unity_asset_bundle.directory_info.len());
        assert_eq!(0, unity_asset_bundle.directory_info[0].offset);
        assert_eq!(4512, unity_asset_bundle.directory_info[0].size);
        assert_eq!(4, unity_asset_bundle.directory_info[0].flags);
        assert_eq!(
            "CAB-5813386f0ea15049abeb5a688d9031d3",
            unity_asset_bundle.directory_info[0].path
        );
    }
}
