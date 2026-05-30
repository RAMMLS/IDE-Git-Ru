//! Binary packfile support for Aura object transport.
//!
//! The format is intentionally simple and deterministic:
//! - 8-byte magic: `AURAPACK`
//! - u32 version
//! - branch name length + bytes
//! - head oid length + bytes
//! - u32 object count
//! - repeated object records:
//!   - kind (u8)
//!   - oid (20 raw bytes)
//!   - body length (u64)
//!   - body bytes

use crate::object::{self, ObjectKind};
use crate::{AuraError, Result};

const PACK_MAGIC: &[u8; 8] = b"AURAPACK";
const PACK_VERSION: u32 = 1;

/// One object entry included in a packfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackObject {
    /// Object id in hex form.
    pub oid: String,
    /// Aura object kind.
    pub kind: ObjectKind,
    /// Raw object body without the loose-object header.
    pub body: Vec<u8>,
}

/// Binary transport envelope used for push/pull/export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packfile {
    /// Branch that the pack targets.
    pub branch: String,
    /// Commit id at the tip of the exported branch.
    pub head: String,
    /// Missing objects required to materialize `head`.
    pub objects: Vec<PackObject>,
}

impl Packfile {
    /// Serializes the packfile into bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.extend_from_slice(PACK_MAGIC);
        out.extend_from_slice(&PACK_VERSION.to_be_bytes());
        write_string(&mut out, &self.branch)?;
        write_string(&mut out, &self.head)?;
        write_u32(&mut out, self.objects.len())?;

        for object in &self.objects {
            out.push(kind_code(object.kind));
            out.extend_from_slice(&decode_oid(&object.oid)?);
            write_u64(&mut out, object.body.len())?;
            out.extend_from_slice(&object.body);
        }

        Ok(out)
    }

    /// Decodes a packfile from bytes and validates every object hash.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let magic = cursor.read_exact(PACK_MAGIC.len())?;
        if magic != PACK_MAGIC {
            return Err(AuraError::InvalidPackfile("invalid magic header".to_string()));
        }

        let version = cursor.read_u32()?;
        if version != PACK_VERSION {
            return Err(AuraError::InvalidPackfile(format!(
                "unsupported packfile version `{version}`"
            )));
        }

        let branch = cursor.read_string()?;
        let head = cursor.read_string()?;
        let object_count = cursor.read_u32()? as usize;
        let mut objects = Vec::with_capacity(object_count);

        for _ in 0..object_count {
            let kind = decode_kind(cursor.read_u8()?)?;
            let oid = hex::encode(cursor.read_exact(20)?);
            let body_len = cursor.read_u64()? as usize;
            let body = cursor.read_exact(body_len)?;
            let computed = object::hash_bytes(kind, &body);
            if computed != oid {
                return Err(AuraError::InvalidPackfile(format!(
                    "object `{oid}` failed hash validation"
                )));
            }
            objects.push(PackObject { oid, kind, body });
        }

        if !cursor.is_eof() {
            return Err(AuraError::InvalidPackfile(
                "trailing bytes after packfile body".to_string(),
            ));
        }

        Ok(Self {
            branch,
            head,
            objects,
        })
    }
}

fn kind_code(kind: ObjectKind) -> u8 {
    match kind {
        ObjectKind::Blob => 1,
        ObjectKind::Tree => 2,
        ObjectKind::Commit => 3,
    }
}

fn decode_kind(value: u8) -> Result<ObjectKind> {
    match value {
        1 => Ok(ObjectKind::Blob),
        2 => Ok(ObjectKind::Tree),
        3 => Ok(ObjectKind::Commit),
        _ => Err(AuraError::InvalidPackfile(format!(
            "unknown object kind `{value}`"
        ))),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<()> {
    write_u32(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_u32(out: &mut Vec<u8>, value: usize) -> Result<()> {
    let value = u32::try_from(value)
        .map_err(|_| AuraError::InvalidPackfile("value does not fit into u32".to_string()))?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn write_u64(out: &mut Vec<u8>, value: usize) -> Result<()> {
    let value = u64::try_from(value)
        .map_err(|_| AuraError::InvalidPackfile("value does not fit into u64".to_string()))?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn decode_oid(oid: &str) -> Result<[u8; 20]> {
    let bytes = hex::decode(oid)?;
    if bytes.len() != 20 {
        return Err(AuraError::InvalidPackfile(format!(
            "invalid object id `{oid}`"
        )));
    }

    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read_exact(&mut self, len: usize) -> Result<Vec<u8>> {
        if self.position + len > self.bytes.len() {
            return Err(AuraError::InvalidPackfile(
                "unexpected end of packfile".to_string(),
            ));
        }

        let chunk = self.bytes[self.position..self.position + len].to_vec();
        self.position += len;
        Ok(chunk)
    }

    fn read_u8(&mut self) -> Result<u8> {
        let chunk = self.read_exact(1)?;
        Ok(chunk[0])
    }

    fn read_u32(&mut self) -> Result<u32> {
        let chunk = self.read_exact(4)?;
        let bytes: [u8; 4] = chunk
            .as_slice()
            .try_into()
            .map_err(|_| AuraError::InvalidPackfile("invalid u32 field".to_string()))?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64> {
        let chunk = self.read_exact(8)?;
        let bytes: [u8; 8] = chunk
            .as_slice()
            .try_into()
            .map_err(|_| AuraError::InvalidPackfile("invalid u64 field".to_string()))?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_exact(len)?;
        String::from_utf8(bytes)
            .map_err(|err| AuraError::InvalidPackfile(format!("invalid utf-8 string: {err}")))
    }

    fn is_eof(&self) -> bool {
        self.position == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{PackObject, Packfile};
    use crate::object::ObjectKind;

    #[test]
    fn packfile_roundtrip_preserves_metadata_and_objects() {
        let pack = Packfile {
            branch: "main".to_string(),
            head: "0123456789abcdef0123456789abcdef01234567".to_string(),
            objects: vec![PackObject {
                oid: crate::object::hash_bytes(ObjectKind::Blob, b"hello\n"),
                kind: ObjectKind::Blob,
                body: b"hello\n".to_vec(),
            }],
        };

        let bytes = pack.to_bytes().expect("pack should encode");
        let decoded = Packfile::from_bytes(&bytes).expect("pack should decode");

        assert_eq!(decoded, pack);
    }

    #[test]
    fn packfile_rejects_object_with_invalid_hash() {
        let pack = Packfile {
            branch: "main".to_string(),
            head: "0123456789abcdef0123456789abcdef01234567".to_string(),
            objects: vec![PackObject {
                oid: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
                kind: ObjectKind::Blob,
                body: b"hello\n".to_vec(),
            }],
        };

        let bytes = pack.to_bytes().expect("pack should encode");
        let error = Packfile::from_bytes(&bytes).expect_err("pack should fail validation");

        assert!(error.to_string().contains("failed hash validation"));
    }
}
