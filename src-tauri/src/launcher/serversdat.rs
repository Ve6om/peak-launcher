//! An instance's own multiplayer list (servers.dat, uncompressed NBT): read,
//! add, remove and reorder entries. Anything Peak doesn't use (server icons,
//! the resource pack choice, other root tags) is kept byte for byte.

use crate::error::{Error, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub address: String,
}

/// One server: its name and address, and its compound's raw payload.
#[derive(Debug, Clone)]
struct Raw {
    entry: Entry,
    payload: Vec<u8>,
}

/// The whole file: the root's name, the root tags other than "servers"
/// (raw), and the servers.
#[derive(Debug, Clone, Default)]
struct Dat {
    root_name: Vec<u8>,
    others: Vec<u8>,
    servers: Vec<Raw>,
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    fn name(&mut self) -> Option<Vec<u8>> {
        let len = u16::from_be_bytes(self.take(2)?.try_into().ok()?) as usize;
        Some(self.take(len)?.to_vec())
    }
    fn skip(&mut self, tag: u8, depth: u8) -> Option<()> {
        if depth > 32 {
            return None;
        }
        match tag {
            1 => self.take(1).map(|_| ()),
            2 => self.take(2).map(|_| ()),
            3 | 5 => self.take(4).map(|_| ()),
            4 | 6 => self.take(8).map(|_| ()),
            7 => {
                let n = self.i32()?.max(0) as usize;
                self.take(n).map(|_| ())
            }
            8 => self.name().map(|_| ()),
            9 => {
                let t = self.u8()?;
                let n = self.i32()?.max(0);
                for _ in 0..n {
                    self.skip(t, depth + 1)?;
                }
                Some(())
            }
            10 => loop {
                let t = self.u8()?;
                if t == 0 {
                    return Some(());
                }
                self.name()?;
                self.skip(t, depth + 1)?;
            },
            11 => {
                let n = self.i32()?.max(0) as usize;
                self.take(n.checked_mul(4)?).map(|_| ())
            }
            12 => {
                let n = self.i32()?.max(0) as usize;
                self.take(n.checked_mul(8)?).map(|_| ())
            }
            _ => None,
        }
    }
}

/// Read one server compound's payload, noting its "name" and "ip".
fn read_server(r: &mut R) -> Option<Raw> {
    let start = r.p;
    let (mut name, mut ip) = (String::new(), String::new());
    loop {
        let t = r.u8()?;
        if t == 0 {
            break;
        }
        let key = r.name()?;
        if t == 8 && (key == b"name" || key == b"ip") {
            let v = String::from_utf8_lossy(&r.name()?).into_owned();
            if key == b"name" {
                name = v;
            } else {
                ip = v;
            }
        } else {
            r.skip(t, 1)?;
        }
    }
    Some(Raw { entry: Entry { name, address: ip }, payload: r.b[start..r.p].to_vec() })
}

fn parse(bytes: &[u8]) -> Option<Dat> {
    let mut r = R { b: bytes, p: 0 };
    if r.u8()? != 10 {
        return None;
    }
    let root_name = r.name()?;
    let mut dat = Dat { root_name, ..Default::default() };
    loop {
        let start = r.p;
        let t = r.u8()?;
        if t == 0 {
            break;
        }
        let key = r.name()?;
        if t == 9 && key == b"servers" {
            let elem = r.u8()?;
            let n = r.i32()?.max(0);
            if elem == 10 {
                for _ in 0..n {
                    dat.servers.push(read_server(&mut r)?);
                }
            } else {
                for _ in 0..n {
                    r.skip(elem, 1)?;
                }
            }
        } else {
            r.skip(t, 1)?;
            dat.others.extend_from_slice(&bytes[start..r.p]);
        }
    }
    Some(dat)
}

fn put_name(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len().min(u16::MAX as usize) as u16).to_be_bytes());
    out.extend_from_slice(&s[..s.len().min(u16::MAX as usize)]);
}

fn new_server(name: &str, address: &str) -> Raw {
    let mut p = Vec::new();
    for (key, value) in [("name", name), ("ip", address)] {
        p.push(8);
        put_name(&mut p, key.as_bytes());
        put_name(&mut p, value.as_bytes());
    }
    p.push(0);
    Raw { entry: Entry { name: name.into(), address: address.into() }, payload: p }
}

fn write(dat: &Dat) -> Vec<u8> {
    let mut out = vec![10];
    put_name(&mut out, &dat.root_name);
    out.extend_from_slice(&dat.others);
    out.push(9);
    put_name(&mut out, b"servers");
    out.push(10);
    out.extend_from_slice(&(dat.servers.len() as i32).to_be_bytes());
    for s in &dat.servers {
        out.extend_from_slice(&s.payload);
    }
    out.push(0);
    out
}

async fn load(file: &Path) -> Result<Dat> {
    match tokio::fs::read(file).await {
        Ok(bytes) => parse(&bytes).ok_or_else(|| Error::msg("This instance's server list (servers.dat) couldn't be read.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Dat::default()),
        Err(e) => Err(e.into()),
    }
}

async fn store(file: &Path, dat: &Dat) -> Result<()> {
    if let Some(dir) = file.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    // Write next to it, then swap, so a crash never leaves half a file.
    let tmp = file.with_extension("dat.peak-tmp");
    tokio::fs::write(&tmp, write(dat)).await?;
    tokio::fs::rename(&tmp, file).await?;
    Ok(())
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

pub async fn list(file: &Path) -> Result<Vec<Entry>> {
    Ok(load(file).await?.servers.into_iter().map(|s| s.entry).collect())
}

/// Add a server to the end of the list, unless its address is already there.
/// Returns whether it was added.
pub async fn add(file: &Path, name: &str, address: &str) -> Result<bool> {
    let mut dat = load(file).await?;
    if dat.servers.iter().any(|s| same(&s.entry.address, address)) {
        return Ok(false);
    }
    let name = if name.trim().is_empty() { address } else { name.trim() };
    dat.servers.push(new_server(name, address.trim()));
    store(file, &dat).await?;
    Ok(true)
}

pub async fn remove(file: &Path, address: &str) -> Result<Vec<Entry>> {
    let mut dat = load(file).await?;
    dat.servers.retain(|s| !same(&s.entry.address, address));
    store(file, &dat).await?;
    Ok(dat.servers.into_iter().map(|s| s.entry).collect())
}

/// Put the servers in the order of the given addresses (others keep their
/// place at the end).
pub async fn reorder(file: &Path, addresses: &[String]) -> Result<Vec<Entry>> {
    let mut dat = load(file).await?;
    let mut ordered = Vec::with_capacity(dat.servers.len());
    for a in addresses {
        if let Some(i) = dat.servers.iter().position(|s| same(&s.entry.address, a)) {
            ordered.push(dat.servers.remove(i));
        }
    }
    ordered.append(&mut dat.servers);
    dat.servers = ordered;
    store(file, &dat).await?;
    Ok(dat.servers.into_iter().map(|s| s.entry).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A servers.dat like Minecraft writes, with an icon and an extra root tag.
    fn sample() -> Vec<u8> {
        let mut b = vec![10, 0, 0];
        // an unrelated root tag, kept as is
        b.push(1);
        put_name(&mut b, b"someFlag");
        b.push(7);
        b.push(9);
        put_name(&mut b, b"servers");
        b.push(10);
        b.extend_from_slice(&2i32.to_be_bytes());
        for (name, ip, icon) in [("Hypixel", "mc.hypixel.net", Some("aWNvbg==")), ("Local", "localhost:25566", None)] {
            b.push(8);
            put_name(&mut b, b"ip");
            put_name(&mut b, ip.as_bytes());
            b.push(8);
            put_name(&mut b, b"name");
            put_name(&mut b, name.as_bytes());
            if let Some(icon) = icon {
                b.push(8);
                put_name(&mut b, b"icon");
                put_name(&mut b, icon.as_bytes());
            }
            b.push(1);
            put_name(&mut b, b"acceptTextures");
            b.push(1);
            b.push(0);
        }
        b.push(0);
        b
    }

    #[test]
    fn round_trips_byte_for_byte() {
        let bytes = sample();
        let dat = parse(&bytes).unwrap();
        assert_eq!(dat.servers.iter().map(|s| s.entry.name.as_str()).collect::<Vec<_>>(), ["Hypixel", "Local"]);
        assert_eq!(dat.servers[0].entry.address, "mc.hypixel.net");
        // Peak always writes "servers" after the other root tags, as here.
        assert_eq!(write(&dat), bytes);
    }

    #[tokio::test]
    async fn add_remove_reorder_keep_the_rest() {
        let dir = std::env::temp_dir().join(format!("peak-serversdat-{}", std::process::id()));
        let file = dir.join("servers.dat");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, sample()).unwrap();

        assert!(add(&file, "Cobble", "play.cobble.net").await.unwrap());
        assert!(!add(&file, "Again", "MC.HYPIXEL.NET").await.unwrap(), "already there");
        let names: Vec<_> = list(&file).await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Hypixel", "Local", "Cobble"]);

        let order = reorder(&file, &["play.cobble.net".into(), "mc.hypixel.net".into()]).await.unwrap();
        assert_eq!(order.iter().map(|e| e.address.as_str()).collect::<Vec<_>>(), ["play.cobble.net", "mc.hypixel.net", "localhost:25566"]);

        remove(&file, "localhost:25566").await.unwrap();
        let bytes = std::fs::read(&file).unwrap();
        let dat = parse(&bytes).unwrap();
        assert_eq!(dat.servers.len(), 2);
        assert!(bytes.windows(8).any(|w| w == b"aWNvbg=="), "Hypixel's icon is kept");
        assert!(bytes.windows(8).any(|w| w == b"someFlag"), "other root tags are kept");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn starts_a_list_when_there_is_none() {
        let dir = std::env::temp_dir().join(format!("peak-serversdat-new-{}", std::process::id()));
        let file = dir.join("servers.dat");
        assert!(list(&file).await.unwrap().is_empty());
        assert!(add(&file, "", "play.example.com").await.unwrap());
        assert_eq!(list(&file).await.unwrap(), vec![Entry { name: "play.example.com".into(), address: "play.example.com".into() }]);
        assert!(parse(b"not nbt").is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
