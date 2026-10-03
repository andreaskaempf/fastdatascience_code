// Sources:
// https://gist.github.com/JalfResi/6287706
// https://dev.to/jones_charles_ad50858dbc0/build-network-proxies-and-reverse-proxies-in-go-a-hands-on-guide-288j

package main

import (
	//"log"
	"fmt"
	"net/http"
	"net/http/httputil"
	"net/url"
)

func main() {

	// Set the remote URL we will proxy to/from
	llm := "https://api.openai.com"
	remote, err := url.Parse(llm)
	if err != nil {
		panic(err)
	}

	// Create handler for reverse proxy
	handler := func(p *httputil.ReverseProxy) func(http.ResponseWriter, *http.Request) {
		return func(w http.ResponseWriter, r *http.Request) {
			//log.Println(r.URL)
			fmt.Println(r.URL)
			r.Host = remote.Host
			//w.Header().Set("X-Ben", "Rad")
			p.ServeHTTP(w, r)
		}
	}

	// Create and activate the proxy
	proxy := httputil.NewSingleHostReverseProxy(remote)
	http.HandleFunc("/", handler(proxy))

	// Listen for calls on designated port, forward them to Google
	fmt.Println("Listening on localhost:8000")
	err = http.ListenAndServe(":8000", nil)
	if err != nil {
		panic(err)
	}
}
