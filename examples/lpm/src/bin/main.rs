/*
 * Copyright 2021 Ari Saha
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the Software), to deal in the
 * Software without restriction, including without limitation the rights to use,
 * copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the
 * Software, and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED AS IS, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
 * FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN
 * AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
 * WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

#[macro_use]
extern crate lazy_static;

extern crate treebitmap;

use capsule::batch::{Batch, Pipeline, Poll};
use capsule::config::load_config;
use capsule::packets::ip::v4::Ipv4;
use capsule::packets::{EtherTypes, Ethernet, Packet};
use capsule::{compose, Mbuf, PortQueue, Runtime};
use colored::*;
use failure::Fallible;
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};
use tracing::{debug, Level};
use tracing_subscriber::fmt;
use treebitmap::IpLookupTable;

lazy_static! {
    static ref LOOKUP_TABLE: Arc<RwLock<IpLookupTable<Ipv4Addr, i32>>> = {
        let mut lpm_table = IpLookupTable::new();

        lpm_table.insert(Ipv4Addr::new(192, 168, 10, 0), 24, 1);
        lpm_table.insert(Ipv4Addr::new(192, 168, 10, 128), 25, 2);

        Arc::new(RwLock::new(lpm_table))
    };
}

#[inline]
fn get_ethernet(packet: Mbuf) -> Fallible<Ethernet> {
    let ethernet = packet.parse::<Ethernet>()?;

    let info_fmt = format!("{:?}", ethernet).magenta().bold();
    println!("{}", info_fmt);

    Ok(ethernet)
}

#[inline]
fn v4_route_lookup(ethernet: &Ethernet) -> Fallible<()> {
    let v4 = ethernet.peek::<Ipv4>()?;
    let info_fmt = format!("{:?}", v4).yellow();
    println!("{}", info_fmt);

    if let Ok(lookup_tbl) = LOOKUP_TABLE.read() {
        let port = lookup_tbl.longest_match(v4.dst()).unwrap().2;
        let info_fmt = format!("{:?}", port).blue();
        println!("{}", info_fmt);
    }

    Ok(())
}

fn install(q: PortQueue) -> impl Pipeline {
    Poll::new(q.clone())
        .map(get_ethernet)
        .group_by(
            |ethernet| ethernet.ether_type(),
            |groups| {
                compose!(groups {
                    EtherTypes::Ipv4 => |group| {
                        group.for_each(v4_route_lookup)
                    }
                });
            },
        )
        .send(q)
}

fn main() -> Fallible<()> {
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = load_config()?;
    debug!(?config);

    Runtime::build(config)?
        .add_pipeline_to_port("eth1", install)?
        .add_pipeline_to_port("eth2", install)?
        .execute()
}
