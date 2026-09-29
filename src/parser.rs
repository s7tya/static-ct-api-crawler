use num_enum::FromPrimitive;
use winnow::{
    Parser, Result,
    binary::{be_u16, be_u24, be_u64, length_and_then, length_take, u8},
    combinator::{eof, fail, repeat, terminated},
    token::take,
};

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, FromPrimitive)]
pub enum ExtensionType {
    LeafIndex = 0,
    #[num_enum(catch_all)]
    Unknown(u8),
}

fn parse_extension_type(input: &mut &[u8]) -> Result<ExtensionType> {
    u8.map(ExtensionType::from).parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Extension<'a> {
    pub extension_type: ExtensionType,
    pub extension_data: &'a [u8],
}

fn parse_extension<'a>(input: &mut &'a [u8]) -> Result<Extension<'a>> {
    (parse_extension_type, length_take(be_u16))
        .map(|(extension_type, extension_data)| Extension {
            extension_type,
            extension_data,
        })
        .verify(|extension| {
            extension.extension_type != ExtensionType::LeafIndex
                || extension.extension_data.len() == 5
        })
        .parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct CtExtensions<'a>(pub Vec<Extension<'a>>);

fn parse_ct_extensions<'a>(input: &mut &'a [u8]) -> Result<CtExtensions<'a>> {
    length_and_then(be_u16, terminated(repeat(0.., parse_extension), eof))
        .verify(|extensions: &Vec<Extension<'_>>| {
            let mut seen = [false; 256];
            extensions.iter().all(|extension| {
                let index = match extension.extension_type {
                    ExtensionType::LeafIndex => 0,
                    ExtensionType::Unknown(value) => value,
                } as usize;
                if seen[index] {
                    return false;
                }
                seen[index] = true;
                true
            })
        })
        .map(CtExtensions)
        .parse_next(input)
}

pub type Fingerprint = [u8; 32];

fn parse_fingerprint(input: &mut &[u8]) -> Result<Fingerprint> {
    take(32usize)
        .try_map(<[u8; 32]>::try_from)
        .parse_next(input)
}

fn parse_certificate_chain<'a>(input: &mut &'a [u8]) -> Result<&'a [Fingerprint]> {
    length_take(be_u16)
        .map(|bytes: &'a [u8]| bytes.as_chunks::<32>())
        .verify(|(_, remainder)| remainder.is_empty())
        .map(|(fingerprints, _)| fingerprints)
        .parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct ASN1Cert<'a>(pub &'a [u8]);

fn parse_asn1_cert<'a>(input: &mut &'a [u8]) -> Result<ASN1Cert<'a>> {
    length_take(be_u24.verify(|len| *len > 0))
        .map(ASN1Cert)
        .parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreCert<'a> {
    pub issuer_key_hash: [u8; 32],
    pub tbs_certificate: TBSCertificate<'a>,
}

fn parse_pre_cert<'a>(input: &mut &'a [u8]) -> Result<PreCert<'a>> {
    (parse_fingerprint, parse_tbs_certificate)
        .map(|(issuer_key_hash, tbs_certificate)| PreCert {
            issuer_key_hash,
            tbs_certificate,
        })
        .parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct TBSCertificate<'a>(pub &'a [u8]);

fn parse_tbs_certificate<'a>(input: &mut &'a [u8]) -> Result<TBSCertificate<'a>> {
    length_take(be_u24.verify(|len| *len > 0))
        .map(TBSCertificate)
        .parse_next(input)
}

#[derive(Debug, PartialEq, Eq)]
pub struct TimestampedEntry<'a, S> {
    pub timestamp: u64,
    pub signed_entry: S,
    pub extensions: CtExtensions<'a>,
}

fn parse_timestamped_entry_body<'a, S>(
    timestamp: u64,
    signed_entry: impl Parser<&'a [u8], S, winnow::error::ContextError>,
) -> impl Parser<&'a [u8], TimestampedEntry<'a, S>, winnow::error::ContextError> {
    (signed_entry, parse_ct_extensions).map(move |(signed_entry, extensions)| TimestampedEntry {
        timestamp,
        signed_entry,
        extensions,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum TileLeaf<'a> {
    X509Entry {
        timestamped_entry: TimestampedEntry<'a, ASN1Cert<'a>>,
        certificate_chain: &'a [Fingerprint],
    },
    PrecertEntry {
        timestamped_entry: TimestampedEntry<'a, PreCert<'a>>,
        pre_certificate: ASN1Cert<'a>,
        certificate_chain: &'a [Fingerprint],
    },
}

pub fn parse_tile_leaf<'a>(input: &mut &'a [u8]) -> Result<TileLeaf<'a>> {
    let timestamp = be_u64.parse_next(input)?;
    let entry_type = be_u16.parse_next(input)?;

    match entry_type {
        0 => (
            parse_timestamped_entry_body(timestamp, parse_asn1_cert),
            parse_certificate_chain,
        )
            .map(
                |(timestamped_entry, certificate_chain)| TileLeaf::X509Entry {
                    timestamped_entry,
                    certificate_chain,
                },
            )
            .parse_next(input),
        1 => (
            parse_timestamped_entry_body(timestamp, parse_pre_cert),
            parse_asn1_cert,
            parse_certificate_chain,
        )
            .map(
                |(timestamped_entry, pre_certificate, certificate_chain)| TileLeaf::PrecertEntry {
                    timestamped_entry,
                    pre_certificate,
                    certificate_chain,
                },
            )
            .parse_next(input),
        _ => fail.parse_next(input),
    }
}
