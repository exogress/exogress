use crate::config_core::parametrized::{Parameter, ParameterOrConfigValue, ParameterSchema};
use crate::config_core::Exception;
use crate::entities::ParameterName;
use core::fmt;
use hashbrown::HashMap;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::convert::TryInto;
use std::marker::PhantomData;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Container<P>
where
    P: ParameterOrConfigValue,
{
    Parameter(ParameterName),
    Value(P),
}

impl<P> Serialize for Container<P>
where
    P: ParameterOrConfigValue,
{
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            Container::Parameter(param) => serializer.serialize_str(format!("@{}", param).as_str()),
            Container::Value(val) => val.serialize(serializer),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parameter {_0} is not defined")]
    ParamNotDefined(ParameterName),

    #[error(
        "parameter schema mismatch. Expected {}, provided {}",
        expected,
        provided
    )]
    SchemaMismatch {
        expected: ParameterSchema,
        provided: ParameterSchema,
    },
}

impl Error {
    pub fn to_exception(&self) -> (Exception, HashMap<SmolStr, SmolStr>) {
        let mut data = HashMap::new();
        match self {
            Error::ParamNotDefined(param) => {
                data.insert("parameter".into(), param.to_string().into());
                (
                    "config-error:parameter-not-defined".try_into().unwrap(),
                    data,
                )
            }
            Error::SchemaMismatch { expected, provided } => {
                data.insert(
                    "expected_parameter_schema".into(),
                    expected.to_string().into(),
                );
                data.insert(
                    "provided_parameter_schema".into(),
                    provided.to_string().into(),
                );

                ("config-error:schema-mismatch".try_into().unwrap(), data)
            }
        }
    }
}

impl<P> Container<P>
where
    P: ParameterOrConfigValue,
{
    pub fn resolve(self, params: &HashMap<ParameterName, Parameter>) -> Result<P, Error> {
        match self {
            Container::Parameter(param) => {
                let found = params
                    .get(&param)
                    .ok_or_else(|| Error::ParamNotDefined(param))?
                    .clone();

                let provided_schema = found.schema();

                found.try_into().map_err(|_| Error::SchemaMismatch {
                    expected: P::schema(),
                    provided: provided_schema,
                })
            }
            Container::Value(v) => Ok(v),
        }
    }
}

struct ContainerVisitor<P>
where
    P: ParameterOrConfigValue,
{
    marker: PhantomData<P>,
}

impl<'de, P> Visitor<'de> for ContainerVisitor<P>
where
    P: ParameterOrConfigValue,
{
    type Value = Container<P>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "@param_name or expected value schema",)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if let Some(param_name) = value.strip_prefix("@") {
            Ok(Container::Parameter(
                param_name
                    .parse()
                    .map_err(|_| de::Error::custom("bad parameter name"))?,
            ))
        } else {
            Err(de::Error::custom("param name should start with '@'"))
        }
    }

    fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        Ok(Container::Value(Deserialize::deserialize(
            de::value::MapAccessDeserializer::new(map),
        )?))
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Ok(Container::Value(Deserialize::deserialize(
            de::value::SeqAccessDeserializer::new(seq),
        )?))
    }
}

impl<'de, P> Deserialize<'de> for Container<P>
where
    P: ParameterOrConfigValue,
{
    fn deserialize<D>(deserializer: D) -> Result<Container<P>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ContainerVisitor {
            marker: PhantomData::default(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_core::parametrized::acl::{Acl, AclEntry};

    #[test]
    fn test_serialize_param_name() {
        let p = Container::Parameter::<Acl>("param".parse().unwrap());
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!("\"@param\"", s);
    }

    #[test]
    fn test_serialize_value() {
        let p = Container::Value(Acl(vec![AclEntry::Allow {
            identity: "username".into(),
        }]));
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!("[{\"allow\":\"username\"}]", s);
    }

    #[test]
    fn test_deserialize_param_name() {
        let s: Container<Acl> = serde_json::from_str("\"@param\"").unwrap();
        assert_eq!(s, Container::<Acl>::Parameter("param".parse().unwrap()));
    }

    #[test]
    fn test_deserialize_value() {
        let s: Container<Acl> = serde_json::from_str("[{\"allow\":\"username\"}]").unwrap();
        assert_eq!(
            s,
            Container::<Acl>::Value(Acl(vec![AclEntry::Allow {
                identity: "username".into()
            }]))
        );
    }
}
