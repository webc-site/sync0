use std::{cell::OnceCell, future::Future, io, path::Path};

use compio::{BufResult, fs::OpenOptions, io::AsyncWriteAtExt, runtime::Runtime};

thread_local! {
  static LOCAL_RT: OnceCell<Runtime> = const { OnceCell::new() };
}

/// 在线程局部 `compio` Runtime 上执行异步 I/O 任务（每线程仅创建一次 `io_uring` / `kqueue` / `IOCP` 实例）
fn run_compio<T, F: Future<Output = io::Result<T>>>(fut: F) -> io::Result<T> {
  LOCAL_RT.with(|cell| {
    if let Some(rt) = cell.get() {
      return rt.block_on(fut);
    }
    let rt = Runtime::new()?;
    cell.get_or_init(|| rt).block_on(fut)
  })
}

/// 利用 `compio` 跨平台完成式异步 I/O (`io_uring` / `kqueue` / `IOCP`) 批量创建并定位写入 (`WriteAt`) 小文件切片，全程复用单块缓冲区
pub fn flush_memory_slices_compio<'a, F>(count: usize, get_item: F) -> io::Result<()>
where
  F: Fn(usize) -> (&'a Path, &'a [u8]),
{
  if count == 0 {
    return Ok(());
  }
  run_compio(async move {
    let mut reusable_buf = Vec::with_capacity(64 * 1024);
    for i in 0..count {
      let (path, data) = get_item(i);
      reusable_buf.clear();
      reusable_buf.extend_from_slice(data);
      let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .await?;
      let BufResult(res, returned_buf) = file.write_all_at(reusable_buf, 0).await;
      res?;
      reusable_buf = returned_buf;
    }
    Ok(())
  })
}
