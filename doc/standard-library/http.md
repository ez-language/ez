# http

The `http` module allows sending HTTP requests and handling responses.

## Functions

-   `get(url: string) -> Result[Response, HttpError]`  
    Sends a GET request to the specified URL.

-   `post(url: string, data: string) -> Result[Response, HttpError]`  
    Sends a POST request with `data` to the specified URL.

-   `put(url: string, data: string) -> Result[Response, HttpError]`  
    Sends a PUT request with `data` to the specified URL.

-   `delete(url: string) -> Result[Response, HttpError]`  
    Sends a DELETE request to the specified URL.

## The `Response` Type

The response to a request has these fields:

-   `status: int`  
    The HTTP status code of the response.

-   `body: string`  
    The body content of the response.

A response with an error status (such as `404` or `500`) is still `Ok(Response)`: the request reached the server. Only failures to communicate are errors.

## Errors

```ez
type HttpError = {
    InvalidUrl(url: string)
    ConnectionFailed(url: string, message: string)
    Timeout(url: string)
    Tls(url: string, message: string)
    InvalidResponse(url: string, message: string)
}
```

## Example

```ez
import http

match (http.get('https://example.com/api')) {
    Ok(res) => if (res.status == 200) {
        print(res.body)
    }
    Err(ConnectionFailed(url, message)) => print(`cannot reach ${url}: ${message}`)
    Err(error) => print(error)
}
```
