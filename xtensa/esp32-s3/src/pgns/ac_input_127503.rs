use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use defmt::{info, Debug2Format};
use korri_n2k::protocol::{managment::address_manager::AddressManager, messages::Pgn127503};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

/// Tâche d'émission périodique du PGN 127503 (AC Input Status).
///
/// Cette tâche démontre l'utilisation de l'API simplifiée `send_pgn` qui gère
/// automatiquement :
/// - La sérialisation du PGN
/// - La segmentation Fast Packet multi-trames
/// - Les délais inter-frame (2ms) pour éviter la saturation du buffer TWAI
/// - Le timeout sur chaque envoi (100ms)
///
/// # Comportement
///
/// Envoie un message PGN 127503 toutes les 1 seconde avec :
/// - Instance : 0
/// - Nombre de lignes AC : 1
/// - Broadcast (destination = None)
///
/// # Conformité NMEA2000
///
/// Le PGN 127503 est un message Fast Packet qui peut générer plusieurs trames.
/// Les délais inter-frame automatiques évitent les problèmes de saturation
/// du buffer TX limité (3 frames) de l'ESP32 TWAI.
#[embassy_executor::task]
pub async fn task_ac_input_127503(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));

    loop {
        ticker.next().await;

        // Créer le message PGN 127503
        let mut ac_input_pgn = Pgn127503::new();
        ac_input_pgn.instance = 210;
        ac_input_pgn.number_of_lines = 185;

        // Envoi simplifié avec gestion automatique Fast Packet et délais
        {
            let mut mgr = manager.lock().await;
            match mgr.send_pgn(&ac_input_pgn, 127503, None).await {
                Ok(_) => {
                    info!("PGN 127503 sent successfully");
                }
                Err(e) => {
                    info!("Error sending PGN 127503: {:?}", Debug2Format(&e));
                }
            }
        }
    }
}
