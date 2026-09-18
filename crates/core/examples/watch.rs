use futures_lite::StreamExt;

fn main() {
    zbus::block_on(async {
        let conn = zinnia_core::volumes::udisks::connect().await.expect("system bus");
        let mut changes = zinnia_core::volumes::watch(&conn).await.expect("watch");
        eprintln!("watching udisks2; press Ctrl+C to stop");
        while let Some(change) = changes.next().await {
            println!("{change:?}");
        }
    });
}
