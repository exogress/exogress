use crate::{
    config_core::{
        referenced::{Parameter, ParameterSchema, ReferencedConfigValue},
        refinable::{NonExistingSharedEntity, RefinableSet, SharedEntity},
        Scope,
    },
    entities::{
        exceptions,
        schemars::{gen::SchemaGenerator, schema::Schema},
        Exception, ParameterName, MAX_STRING_IDENTIFIER_LENGTH, MIN_STRING_IDENTIFIER_LENGTH,
        STRING_ENTITY_REGEXP_PATTERN_NON_FIXED,
    },
};
use core::fmt;
use hashbrown::HashMap;
use schemars::{
    schema::{InstanceType, Metadata, SchemaObject, StringValidation, SubschemaValidation},
    JsonSchema,
};
use serde::{
    de,
    de::{MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use smol_str::SmolStr;
use std::{convert::TryInto, marker::PhantomData};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Container<P, R = NonExistingSharedEntity>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    Shared(R),
    Parameter(ParameterName),
    Value(P),
}

impl<P, R> JsonSchema for Container<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    fn schema_name() -> String {
        format!(
            "{}_or_ParameterName_or_{}",
            R::schema_name(),
            P::schema_name()
        )
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(format!("Either value or parameter name or entity name")),
                description: Some(format!(
                    "You may supply the desired object or the parameter name (starting with @)"
                )),
                ..Default::default()
            })),
            instance_type: Some(
                vec![
                    InstanceType::String.into(),
                    InstanceType::Object.into(),
                    InstanceType::Array.into(),
                ]
                .into(),
            ),
            subschemas: Some(Box::new(SubschemaValidation {
                one_of: Some(vec![
                    SchemaObject {
                        instance_type: Some(InstanceType::String.into()),
                        string: Some(Box::new(StringValidation {
                            max_length: Some((MAX_STRING_IDENTIFIER_LENGTH + 1) as u32),
                            min_length: Some((MIN_STRING_IDENTIFIER_LENGTH + 1) as u32),
                            pattern: Some(String::from(format!(
                                "^@{}$",
                                STRING_ENTITY_REGEXP_PATTERN_NON_FIXED
                            ))),
                        })),
                        ..Default::default()
                    }
                    .into(),
                    gen.subschema_for::<P>(),
                    gen.subschema_for::<R>(),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl<P, R> Serialize for Container<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            Container::Parameter(param) => serializer.serialize_str(format!("@{}", param).as_str()),
            Container::Value(val) => val.serialize(serializer),
            Container::Shared(reference) => {
                serializer.serialize_str(format!("{}", reference).as_str())
            }
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("name {_0} is not defined")]
    NameNotDefined(SmolStr),

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
                (exceptions::CONFIG_PARAMETER_NOT_DEFINED.clone(), data)
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

                (exceptions::CONFIG_SCHEMA_MISMATCH.clone(), data)
            }
            Error::NameNotDefined(name) => {
                data.insert("reference-name".into(), name.to_string().into());
                (exceptions::CONFIG_REFERENCE_NAME_NOT_DEFINED.clone(), data)
            }
        }
    }
}

impl<P, R> Container<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity<Value = P>,
{
    pub fn resolve(
        self,
        params: &HashMap<ParameterName, Parameter>,
        refinable_set: &RefinableSet,
        scope: &Scope,
    ) -> Result<P, Error> {
        match self {
            Container::Parameter(param) => {
                let found = params
                    .get(&param)
                    .ok_or(Error::ParamNotDefined(param))?
                    .clone();

                let provided_schema = found.schema();

                found.try_into().map_err(|_| Error::SchemaMismatch {
                    expected: P::schema(),
                    provided: provided_schema,
                })
            }
            Container::Value(v) => Ok(v),
            Container::Shared(ref_name) => Ok(ref_name
                .get_refined(refinable_set, &scope)
                .ok_or_else(|| Error::NameNotDefined(ref_name.to_string().into()))?
                .0),
        }
    }
}

impl<P, R> Container<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    pub fn resolve_non_referenced(
        self,
        params: &HashMap<ParameterName, Parameter>,
    ) -> Result<P, Error> {
        match self {
            Container::Parameter(param) => {
                let found = params
                    .get(&param)
                    .ok_or(Error::ParamNotDefined(param))?
                    .clone();

                let provided_schema = found.schema();

                found.try_into().map_err(|_| Error::SchemaMismatch {
                    expected: P::schema(),
                    provided: provided_schema,
                })
            }
            Container::Value(v) => Ok(v),
            Container::Shared(_) => {
                unreachable!()
            }
        }
    }
}

struct ContainerVisitor<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    marker_1: PhantomData<P>,
    marker_2: PhantomData<R>,
}

impl<'de, P, R> Visitor<'de> for ContainerVisitor<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    type Value = Container<P, R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "@param_name, entity_name or expected value schema",
        )
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
            Ok(Container::Shared(
                value
                    .parse()
                    .map_err(|_| de::Error::custom("bad entity name"))?,
            ))
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

impl<'de, P, R> Deserialize<'de> for Container<P, R>
where
    P: ReferencedConfigValue,
    R: SharedEntity,
{
    fn deserialize<D>(deserializer: D) -> Result<Container<P, R>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ContainerVisitor {
            marker_1: PhantomData::default(),
            marker_2: PhantomData::default(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_core::referenced::acl::{Acl, AclEntry};

    pub type NonSharedContainer<P> = Container<P, NonExistingSharedEntity>;

    #[test]
    fn test_serialize_param_name() {
        let p = NonSharedContainer::<Acl>::Parameter("param".parse().unwrap());
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!("\"@param\"", s);
    }

    #[test]
    fn test_serialize_value() {
        let p = NonSharedContainer::Value(Acl(vec![AclEntry::Allow {
            identity: "username".into(),
        }]));
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!("[{\"allow\":\"username\"}]", s);
    }

    #[test]
    fn test_deserialize_param_name() {
        let s: NonSharedContainer<Acl> = serde_json::from_str("\"@param\"").unwrap();
        assert_eq!(
            s,
            NonSharedContainer::<Acl>::Parameter("param".parse().unwrap())
        );
    }

    #[test]
    fn test_deserialize_value() {
        let s: NonSharedContainer<Acl> =
            serde_json::from_str("[{\"allow\":\"username\"}]").unwrap();
        assert_eq!(
            s,
            NonSharedContainer::<Acl>::Value(Acl(vec![AclEntry::Allow {
                identity: "username".into()
            }]))
        );
    }
}
