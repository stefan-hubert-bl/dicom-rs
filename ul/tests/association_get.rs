use dicom_ul::{
    association::{
        client::ClientAssociationOptions, Association, ServerAssociationOptions, SyncAssociation,
    },
    pdu::{Pdu, PresentationContextNegotiated, ScpRoleSupport, ScuRoleSupport, UserVariableItem},
};
use std::net::SocketAddr;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;
static REQUESTOR_AE_TITLE: &str = "C-GET-REQUESTOR";
static ACCEPTOR_AE_TITLE: &str = "C-GET-ACCEPTOR";

fn role_selection_items_accepted_for(
    association: &impl Association,
) -> Vec<(&str, &ScuRoleSupport, &ScpRoleSupport)> {
    pdu::role_selection_items_in(association.user_variables())
}

const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";

const SOP_CLASS_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
const SOP_CLASS_DIGITAL_MG_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1.2";
const SOP_CLASS_STUDY_ROOT_QR_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";

fn spawn_association_acceptor() -> Result<(std::thread::JoinHandle<Result<()>>, SocketAddr)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp: ServerAssociationOptions<'_, dicom_ul::association::server::AcceptCalledAeTitle> =
        ServerAssociationOptions::new()
            .accept_called_ae_title()
            .ae_title(ACCEPTOR_AE_TITLE)
            .with_abstract_syntax(SOP_CLASS_STUDY_ROOT_QR_GET)
            .with_abstract_syntax(SOP_CLASS_MR_IMAGE_STORAGE)
            .with_abstract_syntax(SOP_CLASS_DIGITAL_MG_STORAGE);

    let h = std::thread::spawn(move || -> Result<_> {
        let (stream, _addr) = listener.accept()?;
        let mut association = scp.establish(stream)?;

        for pc in association.presentation_contexts() {
            match pc {
                PresentationContextNegotiated {
                    abstract_syntax,
                    transfer_syntax,
                    ..
                } if abstract_syntax == SOP_CLASS_STUDY_ROOT_QR_GET => {
                    // guaranteed to be MR image storage
                    assert_eq!(transfer_syntax, IMPLICIT_VR_LE);
                }
                PresentationContextNegotiated {
                    abstract_syntax,
                    transfer_syntax,
                    ..
                } if abstract_syntax == SOP_CLASS_MR_IMAGE_STORAGE => {
                    // guaranteed to be MR image storage
                    assert_eq!(transfer_syntax, IMPLICIT_VR_LE);
                }
                PresentationContextNegotiated {
                    abstract_syntax,
                    transfer_syntax,
                    ..
                } if abstract_syntax == SOP_CLASS_DIGITAL_MG_STORAGE => {
                    // guaranteed to be MR image storage
                    assert_eq!(transfer_syntax, JPEG_BASELINE);
                }
                _ => panic!("unexpected presentation context {:?}", pc),
            }
        }

        assert_eq!(
            role_selection_items_accepted_for(&association),
            vec![
                (
                    SOP_CLASS_STUDY_ROOT_QR_GET,
                    &ScuRoleSupport::Support,
                    &ScpRoleSupport::NonSupport
                ),
                (
                    SOP_CLASS_MR_IMAGE_STORAGE,
                    &ScuRoleSupport::NonSupport,
                    &ScpRoleSupport::Support
                ),
                (
                    SOP_CLASS_DIGITAL_MG_STORAGE,
                    &ScuRoleSupport::NonSupport,
                    &ScpRoleSupport::Support
                )
            ]
        );

        //assert_eq!(association.client_roles(),
        // handle one release request
        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(())
    });
    Ok((h, addr))
}

/// Run an SCP and an SCU concurrently,
/// negotiate an association with distinct transfer syntaxes
/// and release it.
#[test]
fn test_build_and_establish_association_for_c_get() {
    // - assemble
    let (acceptor_handle, scp_addr) = spawn_association_acceptor().unwrap();

    // - act
    let association = ClientAssociationOptions::new()
        .calling_ae_title(REQUESTOR_AE_TITLE)
        .called_ae_title(ACCEPTOR_AE_TITLE)
        // C-Get
        .with_presentation_context(SOP_CLASS_STUDY_ROOT_QR_GET, vec![IMPLICIT_VR_LE])
        .with_role_selection_item(
            SOP_CLASS_STUDY_ROOT_QR_GET,
            ScuRoleSupport::Support,
            ScpRoleSupport::NonSupport,
        )
        .with_presentation_context(SOP_CLASS_MR_IMAGE_STORAGE, vec![IMPLICIT_VR_LE])
        .with_role_selection_item(
            SOP_CLASS_MR_IMAGE_STORAGE,
            ScuRoleSupport::NonSupport,
            ScpRoleSupport::Support,
        )
        // MG storage, JPEG baseline
        .with_presentation_context(SOP_CLASS_DIGITAL_MG_STORAGE, vec![JPEG_BASELINE])
        .with_role_selection_item(
            SOP_CLASS_DIGITAL_MG_STORAGE,
            ScuRoleSupport::NonSupport,
            ScpRoleSupport::Support,
        )
        .establish(scp_addr)
        .unwrap();

    // - assert (more asserts inside spawn_association_acceptor())
    for pc in association.presentation_contexts() {
        match pc {
            PresentationContextNegotiated {
                abstract_syntax,
                transfer_syntax,
                ..
            } if abstract_syntax == SOP_CLASS_STUDY_ROOT_QR_GET => {
                // guaranteed to be MR image storage
                assert_eq!(transfer_syntax, IMPLICIT_VR_LE);
            }
            PresentationContextNegotiated {
                abstract_syntax,
                transfer_syntax,
                ..
            } if abstract_syntax == SOP_CLASS_MR_IMAGE_STORAGE => {
                // guaranteed to be MR image storage
                assert_eq!(transfer_syntax, IMPLICIT_VR_LE);
            }
            PresentationContextNegotiated {
                abstract_syntax,
                transfer_syntax,
                ..
            } if abstract_syntax == SOP_CLASS_DIGITAL_MG_STORAGE => {
                // guaranteed to be MR image storage
                assert_eq!(transfer_syntax, JPEG_BASELINE);
            }
            _ => panic!("unexpected presentation context {:?}", pc),
        }
    }

    assert_eq!(
        role_selection_items_accepted_for(&association),
        vec![
            (
                SOP_CLASS_STUDY_ROOT_QR_GET,
                &ScuRoleSupport::Support,
                &ScpRoleSupport::NonSupport
            ),
            (
                SOP_CLASS_MR_IMAGE_STORAGE,
                &ScuRoleSupport::NonSupport,
                &ScpRoleSupport::Support
            ),
            (
                SOP_CLASS_DIGITAL_MG_STORAGE,
                &ScuRoleSupport::NonSupport,
                &ScpRoleSupport::Support
            )
        ]
    );

    association
        .release()
        .expect("did not have a peaceful release");

    acceptor_handle
        .join()
        .expect("Couldn't join on the associated thread")
        .expect("Error at the acceptor");
}
