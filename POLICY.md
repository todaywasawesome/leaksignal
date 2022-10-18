
<a href="https://www.leaksignal.com"><p align="center">
  <img src="assets/logo-black-red.png?sanitize=true#gh-light-mode-only" width="800">
  <!--<img src="assets/logo-black-red.png?sanitize=true#gh-dark-mode-only" width="800">-->
</p></a>

## LeakSignal Policy

LeakSignal Policies (LS policies or policies) are YAML configuration files that drive how LeakSignal scans for sensitive data.
There are two primary sections of LS policies, `categories` of matchers and `endpoints`.

### Categories

Categories map a human concept of sensitive data to concrete ways to match that data. Regexes, literal strings, regexes near other regexes, etc, are all examples of this.

#### Matcher Category
A matcher category matches on data on a response body. They can be regexes, raw values, exceptions (the `ignore` section), or accelerated native matchers. All individual matching strategies are considered individually, and a match by any matching strategy constitutes a match of the category.


##### Writing regexes

The `fancy_regex` Rust crate has poor performance when used with `lookaround`, i.e. `lookahead` or `lookbehind` regex features. It's advised to instead capture the area around the interesting sensitive data, then strip it out via the `regex_strip` field shown below.

Since the policies are written in YAML, the any backslashes in a regex must be escaped. I.e. `\d` would be invalid, where `\\d` would match a digit group as expected. It may be cleaner/more readable to prefer manually specifying groups where possible. I.e. `[0-9]` as opposed to `\\d`.

##### Email regex with an ignored email
This example matches emails, but ignores the specific email `someone@example.com`

```
categories:
  email:
    Matchers:
      regexes:
        - "[a-zA-Z0-9_.+-]{2,}@[a-zA-Z0-9-]{3,}\\.[a-zA-Z0-9-.]{2,}"
      ignore:
        - someone@example.com
```

##### Suspicious strings
This example matches some suspicious string literals. These are usually used on JSON or other semistructured data formats. In practice, they might be broken out into multiple rules for more detailed reporting.

```
categories:
  suspicious_strings:
    Matchers:
      raw:
        - credit_card
        - social_security_number
        - password_hash
```

##### Unformatted phone numbers
This example matches 10 digit unformatted phone numbers. It checks for the initial and last character being a non-digit then strips the first and last character from the match data with the `regex_strip` field. This is done to avoid using lookahead/lookbehind, which can be much slower.
```
categories:
  phone_number_correlate:
    Matchers:
      regex_strip: 1
      regexes:
        - "[^0-9][0-9]{10}[^0-9]"
```

#### Correlate Category
A correlate category composes two other categories (generally matcher category), and only signals a match if the two match within a certain distance of one another.

##### Unformatted phone numbers near "phone"
This example matches 10 digit unformatted phone numbers within 64 bytes of the "phone" string. The `interest` field denotes which of the two groups should be reported as interesting, or if omitted, both groups **and all characters inbetween** are considered as matched.

```
categories:
  phone_number_correlate:
    Correlate:
      group1:
        regex_strip: 1
        regexes:
          - "[^0-9][0-9]{10}[^0-9]"
      group2:
        raw:
          - phone
      interest: group1
      max_distance: 64
```

Notably, these groups can also be references to other matching rules:

```
categories:
  phone_number:
    Matchers:
      regex_strip: 1
      regexes:
        - "[^0-9][0-9]{10}[^0-9]"
  phone_number_correlate:
    Correlate:
      group1: phone_number
      group2:
        raw:
          - phone
      interest: group1
      max_distance: 64
```

#### Rematch Category
A rematch category composes two other categories (generally matcher category). It matches the first category, then the second category in sequence. This can be used to improve performance with complex regex.

It is currently disabled, but support will be re-enabled in the future.

##### Credit card validity check
This example matches 4 dash-separated groups of 4 digits (a credit card). It then validates against a regex that checks for a valid credit card.

```
categories:
  credit_card_rematched:
    Rematch:
      target:
        regex_strip: 1
        regexes:
          - "[^0-9]\\d{4}[\\s.-]\\d{4}[\\s.-]\\d{4}[\\s.-]\\d{4}"[^0-9]"
          - "[^0-9]\\d{16}[^0-9]"
      rematcher:
        regexes:
          - "(?:4[0-9]{12}(?:[0-9]{3})?|[25][1-7][0-9]{14}|6(?:011|5[0-9][0-9])[0-9]{12}|3[47][0-9]{13}|3(?:0[0-5]|[68][0-9])[0-9]{11}|(?:2131|1800|35\\d{3})\\d{11})"
```

### Endpoints

Endpoints in LS policies are a mapping from one or more path globs to a series of matching rules. A single request/response pair can match multiple path globs, and therefore be evaluated with multiple groups. A given matching rule is never called twice.

#### Path Globs
Path globs are similar to a host-prefixed HTTP path.

##### Components

A path glob is made up of forward-slash-separated components, with no trailing or leading slash. The first component is protocol specific, in HTTP/gRPC it's the `:authority` or `Host` header. The rest of the components are the HTTP path, not including the query string.

Each component can be one of the following:
* `*`: Matches any single component.
* `**`: Matches an arbitrary number of components (0 or more). This is the only path glob component that can match a variable number of components.
* `#<regex>`: Matches the given regex against the component. Forward slashes are not allowed.
* `*suffix`: Matches if the component ends with the suffix
* `prefix*`: Matches if the component starts with the prefix
* `*within*`: Matches if the component contains the text
* `text`: Matches if the component equals the text.

##### Examples

```
# matches any path
**
# matches the path /foo on any hostname
*/foo
# matches any path on the 'example.com' hostname
example.com/**
# matches a parameter component
# i.e. example.com/product/123 OR example.com/product/ABC
example.com/product/*
# matches a regex limited component
# i.e. example.com/product/123 BUT NOT example.com/product/ABC
example.com/product/#[0-9]+
# matches any path ending in '.php'
# the last component must end with '.php', but the rest of the components are ignored
**/*.php
```

#### Endpoint Configuration

An individual endpoint block is composed of one or more of path globs for matching paths and a configuration set for different matching rules.

##### Schema of EndpointConfig

* `matches: String | String[]`: Single string for a single path glob, or an array of strings representing path globs to match against.
* `config: Map<String, MatchConfig>`: A map of matcher rule/category names to a configuration. Sometimes just an empty object. A category name's presence enables it for requests that match this endpoint configuration.
* `token_extractor: TokenExtractionConfig?`: Configuration for token extraction.
* `report_style: DataReportStyle?`: General report style for requests that match this endpoint configuration. Can be overridden by individual `MatchConfig` in `config`, and overrides `report_style` at the root-level of the policy. This is flattened into the `EndpointConfig`, so the `report_style` key is not present.

##### Schema of MatchConfig

* `action: 'ignore' | 'alert'`: Sets action upon matching. `ignore` does nothing. `alert` forwards the match upstream. Defaults to `alert`.
* `content_types: ContentType | ContentType[]`: `ContentType` can be `json` or `html`. Not specifying `content_types` doesn't filter responses on content type.
* `contexts: MatchContext | MatchContext[]`: `MatchContext` can be `keys` or `values`. Interpretation depends on `content_types`.
* `alert: AlertConfig`: The configuration of alerts for this endpoint. Onlu functional when policy is served through LeakSignal Command.
* `ignore: String[]`: A set of strings to ignore if matched in this path context.
* `report_style: DataReportStyle?`: Specific report style for requests that match this match configuration. Overrides `report_style` at the root-level of the policy and in `EndpointConfig`. This is flattened into the `EndpointConfig`, so the `report_style` key is not present.

##### Schema of TokenExtractionConfig

* `location: 'request' | 'request_cookie' | 'response'`: Which location to pull a token from, request headers, request cookies, or response headers. `response` headers are preferred as they are immune to client-side forgery.
* `header: String`: The specific header/cookie name to extract
* `regex: String?`: A regex to match to validate a token. If there is a first capture group, it is returned; otherwise, the entire regex match is returned. If no `regex` is specified, the entire captures token is returned.

##### Schema of DataReportStyle
For `EndpointConfig` and `MatchConfig`, this type is flattened into its parent containers. For the root policy level, it is not flattened.

`report_style: 'raw' | 'partial_sha256' | 'sha256' | 'none'`: How to report the matched data upstream, if at all. Default is `none` in `EndpointConfig` and `MatchConfig`, and `raw` at the root policy level.
`report_bits: usize`: Only specified if `report_style` is `partial_sha256`. Sets number of bits out of the SHA-256 hash to report. Must be between 0 and 256 exclusive.

##### Schema of AlertConfig

* `per_request: usize?`: Sets the minimum number of unique matches required within a single response to fire an alert.
* `per_5min_by_ip: usize?`: Sets the minimum number of unique matches by a unique IP within a 5 minute span to fire an alert. If the `report_style` is `none`, then any matches are considered unique.
* `per_5min_by_token: usize?`: Sets the minimum number of unique matches by a unique token within a 5 minute span to fire an alert. If the `report_style` is `none`, then any matches are considered unique.

##### Example

Example:
```
endpoints:
  - matches: "**"
    config:
      name_key:
        content_types: json
        contexts: keys
      credit_card:
        report_style: partial_sha256
        report_bits: 32
        alert:
          per_request: 1
      ssn:
        report_style: partial_sha256
        report_bits: 24
        alert:
          per_5min_by_ip: 3
          per_5min_by_token: 3
      email: {}
      phone_number: {}
      address: {}
      date_of_birth: {}
      email: {}
      address: {}
      phone_number_correlate: {}
```

### Overall Policy Schema

* `categories: Map<String, MatchCategory>`: All of the matchable categories
* `endpoints: EndpointConfig[]`: All of the configured endpoints
* `collected_request_headers: String[]`: All request headers that are not redacted. Default list is available at `leakpolicy/src/lib.rs`.
* `collected_response_headers: String[]`: All response headers that are not redacted. Default list is available at `leakpolicy/src/lib.rs`.
* `body_collection_rate`: A floating point ratio between 0.0 and 1.0 denoting how often responses are to be recorded in their entirety __without redaction__. Defaults to 0.0. Responses are not able to be retrieved or analyzed at this time, pending further implementation.
* `report_style: DataReportStyle`: Global report style, defaults to `raw`
