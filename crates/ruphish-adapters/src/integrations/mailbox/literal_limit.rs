//! Bound IMAP literals before the protocol parser can allocate from a declaration.
use std::{
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader, ReadBuf};

pub(crate) const MAX_MESSAGE: usize = 2 * 1024 * 1024;
pub(crate) struct Limited<T> {
    inner: BufReader<T>,
    line: Vec<u8>,
    offset: usize,
    literal: usize,
    ready: bool,
    total: usize,
}
impl<T: AsyncRead> Limited<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner: BufReader::new(inner),
            line: Vec::new(),
            offset: 0,
            literal: 0,
            ready: false,
            total: 0,
        }
    }
    pub(crate) fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}
impl<T> std::fmt::Debug for Limited<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoundedImapTransport")
    }
}
fn denied() -> io::Error {
    io::Error::other("imap_response_limit")
}
impl<T: AsyncRead + Unpin> AsyncRead for Limited<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let s = self.get_mut();
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if s.ready {
                let n = (s.line.len() - s.offset).min(out.remaining());
                out.put_slice(&s.line[s.offset..s.offset + n]);
                s.offset += n;
                if s.offset == s.line.len() {
                    s.line.clear();
                    s.offset = 0;
                    s.ready = false;
                }
                return Poll::Ready(Ok(()));
            }
            if s.total >= 8 * 1024 * 1024 {
                return Poll::Ready(Err(denied()));
            }
            let bytes = ready!(Pin::new(&mut s.inner).poll_fill_buf(cx))?;
            if bytes.is_empty() {
                return Poll::Ready(if s.line.is_empty() && s.literal == 0 {
                    Ok(())
                } else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                });
            }
            if s.literal > 0 {
                let n = bytes.len().min(s.literal).min(out.remaining());
                out.put_slice(&bytes[..n]);
                Pin::new(&mut s.inner).consume(n);
                s.literal -= n;
                s.total += n;
                return Poll::Ready(Ok(()));
            }
            let n = bytes
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |i| i + 1);
            s.line.extend_from_slice(&bytes[..n]);
            Pin::new(&mut s.inner).consume(n);
            s.total += n;
            if s.line.len() > 64 * 1024 {
                return Poll::Ready(Err(denied()));
            }
            if s.line.last() == Some(&b'\n') {
                let tail = s.line.strip_suffix(b"\r\n").unwrap_or(&s.line);
                if tail.last() == Some(&b'}') {
                    let start = tail.iter().rposition(|b| *b == b'{').ok_or_else(denied)?;
                    let value = tail[start + 1..tail.len() - 1]
                        .strip_suffix(b"+")
                        .unwrap_or(&tail[start + 1..tail.len() - 1]);
                    let length = std::str::from_utf8(value)
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|n| *n <= MAX_MESSAGE)
                        .ok_or_else(denied)?;
                    s.literal = length;
                }
                s.ready = true;
            }
        }
    }
}
impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for Limited<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, b: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(self.get_mut().inner.get_mut()).poll_write(cx, b)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(self.get_mut().inner.get_mut()).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(self.get_mut().inner.get_mut()).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Bounded protocol fixture assertions")]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    #[tokio::test]
    async fn fragmented_literals_survive_but_oversized_declarations_never_reach_parser() {
        let protocol = b"* 1 FETCH (BODY[] {5}\r\nhello)\r\nA1 OK done\r\n";
        let mut reader = Limited::new(&protocol[..]);
        let mut decoded = Vec::new();
        let mut byte = [0];
        while reader.read(&mut byte).await.unwrap() != 0 {
            decoded.push(byte[0]);
        }
        assert_eq!(decoded, protocol);
        for line in [
            b"* 1 FETCH (BODY[] {999999999}\r\n".as_slice(),
            b"* 1 FETCH (BODY[] {18446744073709551616}\r\n",
        ] {
            let mut reader = Limited::new(line);
            let mut decoded = Vec::new();
            assert!(reader.read_to_end(&mut decoded).await.is_err());
            assert!(decoded.is_empty());
        }
        let mut reader = Limited::new(&b"* 1 FETCH (BODY[] {20}\r\nshort"[..]);
        assert!(reader.read_to_end(&mut Vec::new()).await.is_err());
    }
}
