# Spec Delta

## ADDED Requirements

### Requirement: niri and cosmic-comp are identified

The Wayland backend SHALL identify niri and cosmic-comp from the Wayland socket's peer binary (`niri`, `cosmic-comp`) and, when the peer is not visible, from `XDG_CURRENT_DESKTOP` (`niri`, `COSMIC`), in the same order of evidence as every other foreign compositor. It SHALL record the identification and its basis like every other one, and SHALL select their window backends and input backends from it.

#### Scenario: niri is identified by its peer binary

- **GIVEN** a niri session whose Wayland peer process is visible as `niri`
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as niri, with the peer process as its basis, and window management SHALL use the niri backend

Unit-tested over the decision function; verified in the local niri lane.

#### Scenario: cosmic-comp is identified from the desktop environment

- **GIVEN** a session whose Wayland peer is not visible and whose `XDG_CURRENT_DESKTOP` is `COSMIC`
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as cosmic-comp, with the session environment as its basis

Unit-tested over the decision function.

#### Scenario: A PlatynUI handshake still wins

- **GIVEN** a PlatynUI compositor session whose environment also names `niri` in `XDG_CURRENT_DESKTOP`
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as PlatynUI, because the handshake is the strongest evidence

Unit-tested over the decision function.

#### Scenario: Input backends follow the identification

- **GIVEN** an identified niri session, and separately an identified cosmic-comp session
- **WHEN** the input backend is selected
- **THEN** niri SHALL use the virtual-input backend
- **AND** cosmic-comp SHALL try the RemoteDesktop portal first and fall back to the virtual-input backend, which offers only the keyboard there, so that pointer actions fail with an unavailable error naming the virtual pointer

Unit-tested over the candidate list; verified in the local lanes.
