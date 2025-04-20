# http

The `http` module allows sending HTTP requests and handling responses.

## Functions

-   `get(url: string) -> Response`  
    Sends a GET request to the specified URL.

-   `post(url: string, data: string) -> Response`  
    Sends a POST request with `data` to the specified URL.

-   `put(url: string, data: string) -> Response`  
    Sends a PUT request with `data` to the specified URL.

-   `delete(url: string) -> Response`  
    Sends a DELETE request to the specified URL.

## Response Object

A response object returned from an HTTP request contains:

-   `status: number`  
    The HTTP status code of the response.

-   `body: string`  
    The body content of the response.

## Example

```ez
import http

res = http.get('https://example.com/api')

if (res.status == 200) {
    print(res.body)
}
```
