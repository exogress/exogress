use crate::entities::Exception;
use lazy_static::lazy_static;

macro_rules! exceptions {
    ($($N:ident => $($segment:expr),+)*) => {
        lazy_static! {
            $( pub static ref $N: Exception = Exception::from_segments(&[$( $segment.parse().unwrap() ),+]);)*
        }
    }
}

exceptions! {
    APPLICATION_FIREWALL_INJECTION_DETECTED => "application-firewall-error", "injection-detected"

    CONFIG_PARAMETER_NOT_DEFINED => "config-error", "parameter-not-defined"
    CONFIG_REFERENCE_NAME_NOT_DEFINED => "config-error", "reference-name-not-defined"
    CONFIG_SCHEMA_MISMATCH => "config-error", "schema-mismatch"

    PROXY_BAD_GATEWAY => "proxy-error", "bad-gateway", "no-healthy-upstreams"
    PROXY_UPSTREAM_UNREACHABLE => "proxy-error", "upstream-unreachable"
    PROXY_UPSTREAM_UNREACHABLE_CONNECTION_REJECTED => "proxy-error", "upstream-unreachable", "connection-rejected"
    PROXY_INSTANCE_UNREACHABLE => "proxy-error", "instance-unreachable"
    PROXY_LOOP_DETECTED => "proxy-error", "loop-detected"
    PROXY_NO_INSTANCES => "proxy-error", "no-instances"
    PROXY_WEBSOCKETS_CONNECTION_ERROR => "proxy-error", "websockets", "connect-error"
    PROXY_WEBSOCKETS_DISABLED => "proxy-error", "websockets", "disabled"

    STATIC_RESPONSE_BAD_ACCEPT_HEADER => "static-response-error", "bad-accept-header"
    STATIC_RESPONSE_NOT_DEFINED => "static-response-error", "not-defined"
    STATIC_RESPONSE_NO_ACCEPT_HEADER => "static-response-error", "no-accept-header"
    STATIC_RESPONSE_RENDER_ERROR => "static-response-error", "render-error"
    STATIC_RESPONSE_REDIRECT_ERROR => "static-response-error", "redirect-error"
}
