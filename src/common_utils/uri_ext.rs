use linked_hash_map::LinkedHashMap;
use url::Url;

pub trait UriExt {
    fn to_url(&self) -> Url;
    fn path_segments(&self) -> Vec<&str>;
    fn query_pairs(&self) -> LinkedHashMap<String, String>;
    fn set_scheme(&mut self, scheme: &str);
    fn set_hostname(&mut self, hostname: &str);
    fn unset_port(&mut self);
    fn clear_query(&mut self);
    fn set_query(&mut self, pairs: LinkedHashMap<String, String>);
    fn clear_segments(&mut self);
    fn push_segment(&mut self, segment: &str);
    fn ensure_trailing_slash(&mut self, set: bool);
}

impl UriExt for http::uri::Uri {
    fn to_url(&self) -> Url {
        self.to_string().parse().unwrap()
    }

    fn path_segments(&self) -> Vec<&str> {
        self.path().split("/").skip(1).collect()
    }

    fn query_pairs(&self) -> LinkedHashMap<String, String> {
        if let Some(query) = self.query() {
            let mut res = LinkedHashMap::new();
            for item in query.split("&") {
                let mut splitted = item.split("=");
                let maybe_key = splitted.next();
                let maybe_value = splitted.next();
                if splitted.next().is_some() {
                    continue;
                };
                if let (Some(key), Some(value)) = (maybe_key, maybe_value) {
                    res.insert(key.to_string(), value.to_string());
                }
            }

            res
        } else {
            Default::default()
        }
    }

    fn unset_port(&mut self) {
        let authority = self.host().unwrap().to_string();
        let mut builder = http::Uri::builder()
            .scheme(self.scheme_str().unwrap())
            .authority(authority.as_str());
        if let Some(p_a_q) = self.path_and_query() {
            builder = builder.path_and_query(p_a_q.as_str());
        }
        *self = builder.build().expect("FIXME");
    }

    fn set_hostname(&mut self, hostname: &str) {
        let mut authority = hostname.to_string();
        if let Some(port) = self.port() {
            authority.push(':');
            authority.push_str(port.as_str());
        }
        let mut builder = http::Uri::builder()
            .scheme(self.scheme_str().unwrap())
            .authority(authority.as_str());
        if let Some(p_a_q) = self.path_and_query() {
            builder = builder.path_and_query(p_a_q.as_str());
        }

        *self = builder.build().expect("FIXME");
    }

    fn set_scheme(&mut self, scheme: &str) {
        let mut builder = http::Uri::builder()
            .scheme(scheme)
            .authority(self.authority().unwrap().as_str());

        if let Some(p_a_q) = self.path_and_query() {
            builder = builder.path_and_query(p_a_q.as_str());
        }

        *self = builder.build().expect("FIXME");
    }

    fn clear_query(&mut self) {
        let builder = http::Uri::builder()
            .path_and_query(self.path())
            .scheme(self.scheme_str().unwrap())
            .authority(self.authority().unwrap().as_str());
        *self = builder.build().expect("FIXME");
    }

    fn ensure_trailing_slash(&mut self, set: bool) {
        let mut new_path_and_query = self.path().to_string();
        let query = self.query();

        if set {
            if !new_path_and_query.ends_with("/") {
                new_path_and_query.push_str("/");
            }
        } else {
            if new_path_and_query.ends_with("/") {
                new_path_and_query.pop();
            }
        }

        if let Some(q) = query {
            new_path_and_query.push_str("?");
            new_path_and_query.push_str(q);
        }

        let builder = http::Uri::builder()
            .path_and_query(new_path_and_query)
            .scheme(self.scheme_str().unwrap())
            .authority(self.authority().unwrap().as_str());

        *self = builder.build().expect("FIXME");
    }

    fn set_query(&mut self, pairs: LinkedHashMap<String, String>) {
        let mut new_path = self.path().to_string();

        if !pairs.is_empty() {
            new_path.push_str("?");
            let new_query_string = pairs
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
            new_path.push_str(new_query_string.as_str());
        }

        let builder = http::Uri::builder()
            .path_and_query(new_path)
            .scheme(self.scheme_str().unwrap())
            .authority(self.authority().unwrap().as_str());

        *self = builder.build().expect("FIXME");
    }

    fn clear_segments(&mut self) {
        let mut builder = http::Uri::builder()
            .scheme(self.scheme_str().unwrap())
            .authority(self.authority().unwrap().as_str());

        let mut path_and_query = "".to_string();
        let current_query = self.query();
        if let Some(q) = current_query {
            path_and_query.push('?');
            path_and_query.push_str(q);
        }

        builder = builder.path_and_query(path_and_query);

        *self = builder.build().expect("FIXME");
    }

    fn push_segment(&mut self, segment: &str) {
        let mut builder = http::Uri::builder()
            .scheme(self.scheme_str().unwrap())
            .authority(self.authority().unwrap().as_str());

        let current_query = self.query();
        let mut current_path = self.path().to_string();

        if !current_path.ends_with('/') {
            current_path.push('/');
        }
        current_path.push_str(segment);

        let mut path_and_query = current_path;
        if let Some(q) = current_query {
            path_and_query.push('?');
            path_and_query.push_str(q);
        }
        builder = builder.path_and_query(path_and_query);

        *self = builder.build().expect("FIXME");
    }
}
