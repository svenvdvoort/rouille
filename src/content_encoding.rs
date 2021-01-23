// Copyright (c) 2016 The Rouille developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use std::str;
use input;
use Request;
use Response;

/// Applies content encoding to the response.
///
/// Analyzes the `Accept-Encoding` header of the request. If one of the encodings is recognized and
/// supported by rouille, it adds a `Content-Encoding` header to the `Response` and encodes its
/// body.
///
/// If the response already has a `Content-Encoding` header, this function is a no-op.
/// If the response has a `Content-Type` header that isn't textual content, this function is a
/// no-op.
///
/// The gzip encoding is supported only if you enable the `gzip` feature of rouille (which is
/// enabled by default).
///
/// # Example
///
/// ```rust
/// use rouille::content_encoding;
/// use rouille::Request;
/// use rouille::Response;
///
/// fn handle(request: &Request) -> Response {
///     content_encoding::apply(request, Response::text("hello world"))
/// }
/// ```
pub fn apply(request: &Request, mut response: Response) -> Response {
    // Only text should be encoded. Otherwise just return.
    if !response_is_text(&response) {
        return response;
    }

    // If any of the response's headers is equal to `Content-Encoding`, ignore the function
    // call and return immediately.
    if response.headers.iter().any(|&(ref key, _)| key.eq_ignore_ascii_case("Content-Encoding")) {
        return response;
    }

    // Now let's get the list of content encodings accepted by the request.
    // The list should be ordered from the most desired to the least desired.
    let encoding_preference = ["br", "gzip", "x-gzip", "identity"];
    let accept_encoding_header = request.header("Accept-Encoding").unwrap_or("");
    if let Some(preferred_index) = input::priority_header_preferred(&accept_encoding_header, encoding_preference.iter().cloned()) {
        match encoding_preference[preferred_index] {
            "br" => brotli(&mut response),
            "gzip" | "x-gzip" => gzip(&mut response),
            _ => (),
        }
    }
    return response;
}

// Returns true if the Content-Type of the response is a type that should be encoded.
// Since encoding is purely an optimisation, it's not a problem if the function sometimes has
// false positives or false negatives.
fn response_is_text(response: &Response) -> bool {
    response.headers.iter().any(|&(ref key, ref value)| {
        if !key.eq_ignore_ascii_case("Content-Type") {
            return false;
        }

        let content_type = value.to_lowercase();
        content_type.starts_with("text/") ||
        content_type.contains("javascript") ||
        content_type.contains("json") ||
        content_type.contains("xml") ||
        content_type.contains("font")
    })
}

#[cfg(feature = "gzip")]
fn gzip(response: &mut Response) {
    use ResponseBody;
    use std::mem;
    use std::io;
    use deflate::deflate_bytes_gzip;

    response.headers.push(("Content-Encoding".into(), "gzip".into()));
    let previous_body = mem::replace(&mut response.data, ResponseBody::empty());
    let (mut raw_data, size) = previous_body.into_reader_and_size();
    let mut src = match size {
        Some(size) => Vec::with_capacity(size),
        None => Vec::new(),
    };
    io::copy(&mut raw_data, &mut src).expect("Failed reading response body while gzipping");
    let zipped = deflate_bytes_gzip(&src);
    response.data = ResponseBody::from_data(zipped);
}

#[cfg(not(feature = "gzip"))]
#[inline]
fn gzip(response: &mut Response) {}

#[cfg(feature = "brotli")]
fn brotli(response: &mut Response) {
    use ResponseBody;
    use std::mem;
    use brotli2::read::BrotliEncoder;

    response.headers.push(("Content-Encoding".into(), "br".into()));
    let previous_body = mem::replace(&mut response.data, ResponseBody::empty());
    let (raw_data, _) = previous_body.into_reader_and_size();
    response.data = ResponseBody::from_reader(BrotliEncoder::new(raw_data, 6));
}

#[cfg(not(feature = "brotli"))]
#[inline]
fn brotli(response: &mut Response) {}

#[cfg(test)]
mod tests {

    // TODO: more tests for encoding stuff
}
