//! The compiler permits ordinary calls; UI alone assigns snapshot semantics.
#![allow(dead_code, unused_imports)]
mod contract;
use contract::*;
use neocogi_com::idl_metadata::{MethodType as M, ValueType as V};
use neocogi_ui_core_contracts::{EventSourceMetadata, UiEventSource};
neocogi_ui_core_contracts::ui_event_sources! { SourceAbi => [GoodAbi]; }

#[test]
fn publication_is_separate_from_abi_and_merges_event_only_values() {
    assert!(SourceAbi::IDL_INTERFACE.values.is_empty());
    let manifest = neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &SourceAbi::EVENT_SOURCE).unwrap();
    assert_eq!(manifest.interface.events[0].fields[0].name, "value");
    assert_eq!(manifest.interface.values.len(), 2);
    assert!(manifest.interface.values.iter().any(|value| value.qualified_name == "proof.admission.Detail"));
    assert_eq!(neocogi_ui_design::DesignWidgetManifest::decode(&manifest.encode().unwrap()).unwrap(), manifest);
    let silent = EventSourceMetadata { interface: &SourceAbi::IDL_INTERFACE, events: &[] };
    let silent = neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &silent).unwrap();
    assert_eq!(silent.interface.iid, manifest.interface.iid);
    assert_eq!(silent.interface.signature, manifest.interface.signature);
    assert!(silent.interface.events.is_empty());
}

#[test]
fn ui_rejects_non_snapshots_and_ambiguous_getters() {
    // Same root IID with incompatible bytes cannot qualify by spelling or nominal identity alone.
    const WRONG_DIGEST: [u8; 32] = {
        let mut bytes = *neocogi_ui_core_contracts::IEventAbi::IDL_INTERFACE.identity.abi_signature;
        bytes[31] ^= 1;
        bytes
    };
    const WRONG_ROOT: neocogi_com::idl_metadata::InterfaceIdentity = neocogi_com::idl_metadata::InterfaceIdentity {
        abi_signature: &WRONG_DIGEST,
        ..neocogi_ui_core_contracts::IEventAbi::IDL_INTERFACE.identity
    };
    static WRONG_CHAIN: [neocogi_com::idl_metadata::InterfaceIdentity; 2] = [GoodAbi::IDL_INTERFACE.identity, WRONG_ROOT];
    let wrong_root = neocogi_com::idl_metadata::InterfaceMetadata {
        chain: &WRONG_CHAIN,
        ..GoodAbi::IDL_INTERFACE
    };
    assert!(neocogi_ui_design::DesignEvent::from_idl(&wrong_root).unwrap_err().contains("derive"));
    for (interface, expected) in [
        (&UnrelatedAbi::IDL_INTERFACE, "derive"),
        (&MutableAbi::IDL_INTERFACE, "immutable"),
        (&ParameterAbi::IDL_INTERFACE, "parameterless"),
        (&TransferAbi::IDL_INTERFACE, "repeatable"),
        (&AmbiguousAbi::IDL_INTERFACE, "ambiguous"),
        (&GenericAmbiguousAbi::IDL_INTERFACE, "ambiguous"),
    ] {
        let error = neocogi_ui_design::DesignEvent::from_idl(interface).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    // Producer identity must match the first chain entry, including byte 31.
    static WRONG_SOURCE: neocogi_com::idl_metadata::InterfaceMetadata = neocogi_com::idl_metadata::InterfaceMetadata {
        chain: &[GoodAbi::IDL_INTERFACE.identity],
        ..SourceAbi::IDL_INTERFACE
    };
    let wrong_source = EventSourceMetadata { interface: &WRONG_SOURCE, events: &[] };
    assert!(neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &wrong_source).unwrap_err().contains("inconsistent"));
    const WRONG_SOURCE_DIGEST: [u8; 32] = {
        let mut bytes = *SourceAbi::IDL_INTERFACE.identity.abi_signature;
        bytes[31] ^= 1;
        bytes
    };
    static WRONG_SOURCE_SIGNATURE: neocogi_com::idl_metadata::InterfaceMetadata = neocogi_com::idl_metadata::InterfaceMetadata {
        identity: neocogi_com::idl_metadata::InterfaceIdentity {
            abi_signature: &WRONG_SOURCE_DIGEST,
            ..SourceAbi::IDL_INTERFACE.identity
        },
        ..SourceAbi::IDL_INTERFACE
    };
    let wrong_signature = EventSourceMetadata { interface: &WRONG_SOURCE_SIGNATURE, events: &[] };
    assert!(neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &wrong_signature).unwrap_err().contains("inconsistent"));
    static EMPTY_SOURCE: neocogi_com::idl_metadata::InterfaceMetadata = neocogi_com::idl_metadata::InterfaceMetadata {
        chain: &[],
        ..SourceAbi::IDL_INTERFACE
    };
    let empty = EventSourceMetadata { interface: &EMPTY_SOURCE, events: &[] };
    assert!(neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &empty).unwrap_err().contains("no COM chain"));
    let repeated = EventSourceMetadata { interface: &SourceAbi::IDL_INTERFACE, events: &[&GoodAbi::IDL_INTERFACE, &GoodAbi::IDL_INTERFACE] };
    assert!(neocogi_ui_design::DesignWidgetManifest::from_source("proof.source", vec![], &repeated).unwrap_err().contains("duplicate"));
}

#[test]
fn ordinary_introspection_preserves_call_only_ownership() {
    let method = &CallsAbi::IDL_INTERFACE.methods[0];
    assert_eq!(method.receiver, neocogi_com::NcAbiReceiver::Mutable);
    assert_eq!(method.parameters.iter().map(|parameter| parameter.name).collect::<Vec<_>>(), ["text", "data", "output", "object"]);
    assert_eq!(method.parameters[0].ty, M::Borrowed(V::Text));
    assert_eq!(method.parameters[1].ty, M::InputBuffer(V::Scalar("bool")));
    assert_eq!(method.parameters[2].ty, M::OutputBuffer(V::Scalar("u32")));
    assert_eq!(method.parameters[3].ty, M::ExclusiveObject("neocogi.ui.core.IEvent"));
    assert_eq!(method.returns, M::Value(V::Unit));
}
