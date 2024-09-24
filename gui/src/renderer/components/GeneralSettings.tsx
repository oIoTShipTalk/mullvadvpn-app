import { useCallback } from 'react';
import styled from 'styled-components';

import { messages } from '../../shared/gettext';
import log from '../../shared/logging';
import { useAppContext } from '../context';
import { useHistory } from '../lib/history';
import { RoutePath } from '../lib/routes';
import { useBoolean } from '../lib/utilityHooks';
import { useSelector } from '../redux/store';
import * as AppButton from './AppButton';
import { AriaDescription, AriaDetails, AriaInput, AriaInputGroup, AriaLabel } from './AriaGroup';
import * as Cell from './cell';
import InfoButton, { InfoIcon } from './InfoButton';
import { BackAction } from './KeyboardNavigation';
import { Layout, SettingsContainer } from './Layout';
import { ModalAlert, ModalAlertType, ModalMessage } from './Modal';
import {
  NavigationBar,
  NavigationContainer,
  NavigationItems,
  NavigationScrollbars,
  TitleBarItem,
} from './NavigationBar';
import SettingsHeader, { HeaderTitle } from './SettingsHeader';

const StyledContent = styled.div({
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  marginBottom: '2px',
});

const StyledCellIcon = styled(Cell.UntintedIcon)({
  marginRight: '8px',
});

const StyledAnimateMapSettingsGroup = styled(Cell.Group)({
  '@media (prefers-reduced-motion: reduce)': {
    display: 'none',
  },
});

const StyledInfoIcon = styled(InfoIcon)({
  marginRight: '16px',
});

export default function GeneralSettings() {
  const { pop } = useHistory();
  const unpinnedWindow = useSelector((state) => state.settings.guiSettings.unpinnedWindow);

  return (
    <BackAction action={pop}>
      <Layout>
        <SettingsContainer>
          <NavigationContainer>
            <NavigationBar>
              <NavigationItems>
                <TitleBarItem>
                  {
                    // TRANSLATORS: Title label in navigation bar
                    messages.pgettext('user-interface-settings-view', 'General')
                  }
                </TitleBarItem>
              </NavigationItems>
            </NavigationBar>

            <NavigationScrollbars>
              <SettingsHeader>
                <HeaderTitle>
                  {messages.pgettext('user-interface-settings-view', 'General')}
                </HeaderTitle>
              </SettingsHeader>

              <StyledContent>
                <Cell.Group>
                  <AutoStart />
                  <AutoConnect />
                </Cell.Group>

                <Cell.Group>
                  <KillSwitchInfo />
                  <LockdownMode />
                </Cell.Group>

                <Cell.Group>
                  <NotificationsSetting />
                </Cell.Group>
                <Cell.Group>
                  <MonochromaticTrayIconSetting />
                </Cell.Group>

                <Cell.Group>
                  <LanguageButton />
                </Cell.Group>

                {(window.env.platform === 'win32' ||
                  (window.env.platform === 'darwin' && window.env.development)) && (
                  <Cell.Group>
                    <UnpinnedWindowSetting />
                  </Cell.Group>
                )}

                {unpinnedWindow && (
                  <Cell.Group>
                    <StartMinimizedSetting />
                  </Cell.Group>
                )}

                <StyledAnimateMapSettingsGroup>
                  <AnimateMapSetting />
                </StyledAnimateMapSettingsGroup>
              </StyledContent>
            </NavigationScrollbars>
          </NavigationContainer>
        </SettingsContainer>
      </Layout>
    </BackAction>
  );
}

function AutoStart() {
  const autoStart = useSelector((state) => state.settings.autoStart);
  const { setAutoStart: setAutoStartImpl } = useAppContext();

  const setAutoStart = useCallback(
    async (autoStart: boolean) => {
      try {
        await setAutoStartImpl(autoStart);
      } catch (e) {
        const error = e as Error;
        log.error(`Cannot set auto-start: ${error.message}`);
      }
    },
    [setAutoStartImpl],
  );

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('vpn-settings-view', 'Launch app on start-up')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={autoStart} onChange={setAutoStart} />
        </AriaInput>
      </Cell.Container>
    </AriaInputGroup>
  );
}

function AutoConnect() {
  const autoConnect = useSelector((state) => state.settings.guiSettings.autoConnect);
  const { setAutoConnect } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('vpn-settings-view', 'Auto-connect')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={autoConnect} onChange={setAutoConnect} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext(
              'vpn-settings-view',
              'Automatically connect to a server when the app launches.',
            )}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function KillSwitchInfo() {
  const [killSwitchInfoVisible, showKillSwitchInfo, hideKillSwitchInfo] = useBoolean(false);

  return (
    <>
      <Cell.CellButton onClick={showKillSwitchInfo}>
        <AriaInputGroup>
          <AriaLabel>
            <Cell.InputLabel>
              {messages.pgettext('vpn-settings-view', 'Kill switch')}
            </Cell.InputLabel>
          </AriaLabel>
          <StyledInfoIcon />
          <AriaInput>
            <Cell.Switch isOn disabled />
          </AriaInput>
        </AriaInputGroup>
      </Cell.CellButton>
      <ModalAlert
        isOpen={killSwitchInfoVisible}
        type={ModalAlertType.info}
        buttons={[
          <AppButton.BlueButton key="back" onClick={hideKillSwitchInfo}>
            {messages.gettext('Got it!')}
          </AppButton.BlueButton>,
        ]}
        close={hideKillSwitchInfo}>
        <ModalMessage>
          {messages.pgettext(
            'vpn-settings-view',
            'This built-in feature prevents your traffic from leaking outside of the VPN tunnel if your network suddenly stops working or if the tunnel fails, it does this by blocking your traffic until your connection is reestablished.',
          )}
        </ModalMessage>
        <ModalMessage>
          {messages.pgettext(
            'vpn-settings-view',
            'The difference between the Kill Switch and Lockdown Mode is that the Kill Switch will prevent any leaks from happening during automatic tunnel reconnects, software crashes and similar accidents. With Lockdown Mode enabled, you must be connected to a Mullvad VPN server to be able to reach the internet. Manually disconnecting or quitting the app will block your connection.',
          )}
        </ModalMessage>
      </ModalAlert>
    </>
  );
}

function LockdownMode() {
  const blockWhenDisconnected = useSelector((state) => state.settings.blockWhenDisconnected);
  const { setBlockWhenDisconnected: setBlockWhenDisconnectedImpl } = useAppContext();

  const [confirmationDialogVisible, showConfirmationDialog, hideConfirmationDialog] =
    useBoolean(false);

  const setBlockWhenDisconnected = useCallback(
    async (blockWhenDisconnected: boolean) => {
      try {
        await setBlockWhenDisconnectedImpl(blockWhenDisconnected);
      } catch (e) {
        const error = e as Error;
        log.error('Failed to update block when disconnected', error.message);
      }
    },
    [setBlockWhenDisconnectedImpl],
  );

  const setLockDownMode = useCallback(
    async (newValue: boolean) => {
      if (newValue) {
        showConfirmationDialog();
      } else {
        await setBlockWhenDisconnected(false);
      }
    },
    [setBlockWhenDisconnected, showConfirmationDialog],
  );

  const confirmLockdownMode = useCallback(async () => {
    hideConfirmationDialog();
    await setBlockWhenDisconnected(true);
  }, [hideConfirmationDialog, setBlockWhenDisconnected]);

  return (
    <>
      <AriaInputGroup>
        <Cell.Container>
          <AriaLabel>
            <Cell.InputLabel>
              {messages.pgettext('vpn-settings-view', 'Lockdown mode')}
            </Cell.InputLabel>
          </AriaLabel>
          <AriaDetails>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'The difference between the Kill Switch and Lockdown Mode is that the Kill Switch will prevent any leaks from happening during automatic tunnel reconnects, software crashes and similar accidents.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'With Lockdown Mode enabled, you must be connected to a Mullvad VPN server to be able to reach the internet. Manually disconnecting or quitting the app will block your connection.',
                )}
              </ModalMessage>
            </InfoButton>
          </AriaDetails>
          <AriaInput>
            <Cell.Switch isOn={blockWhenDisconnected} onChange={setLockDownMode} />
          </AriaInput>
        </Cell.Container>
      </AriaInputGroup>
      <ModalAlert
        isOpen={confirmationDialogVisible}
        type={ModalAlertType.caution}
        buttons={[
          <AppButton.RedButton key="confirm" onClick={confirmLockdownMode}>
            {messages.gettext('Enable anyway')}
          </AppButton.RedButton>,
          <AppButton.BlueButton key="back" onClick={hideConfirmationDialog}>
            {messages.gettext('Back')}
          </AppButton.BlueButton>,
        ]}
        close={hideConfirmationDialog}>
        <ModalMessage>
          {messages.pgettext(
            'vpn-settings-view',
            'Attention: enabling this will always require a Mullvad VPN connection in order to reach the internet.',
          )}
        </ModalMessage>
        <ModalMessage>
          {messages.pgettext(
            'vpn-settings-view',
            'The app’s built-in kill switch is always on. This setting will additionally block the internet if clicking Disconnect or Quit.',
          )}
        </ModalMessage>
      </ModalAlert>
    </>
  );
}

function NotificationsSetting() {
  const enableSystemNotifications = useSelector(
    (state) => state.settings.guiSettings.enableSystemNotifications,
  );
  const { setEnableSystemNotifications } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('user-interface-settings-view', 'Notifications')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={enableSystemNotifications} onChange={setEnableSystemNotifications} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext(
              'user-interface-settings-view',
              'Enable or disable system notifications. The critical notifications will always be displayed.',
            )}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function MonochromaticTrayIconSetting() {
  const monochromaticIcon = useSelector((state) => state.settings.guiSettings.monochromaticIcon);
  const { setMonochromaticIcon } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('user-interface-settings-view', 'Monochromatic tray icon')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={monochromaticIcon} onChange={setMonochromaticIcon} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext(
              'user-interface-settings-view',
              'Use a monochromatic tray icon instead of a colored one.',
            )}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function UnpinnedWindowSetting() {
  const unpinnedWindow = useSelector((state) => state.settings.guiSettings.unpinnedWindow);
  const { setUnpinnedWindow } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('user-interface-settings-view', 'Unpin app from taskbar')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={unpinnedWindow} onChange={setUnpinnedWindow} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext(
              'user-interface-settings-view',
              'Enable to move the app around as a free-standing window.',
            )}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function StartMinimizedSetting() {
  const startMinimized = useSelector((state) => state.settings.guiSettings.startMinimized);
  const { setStartMinimized } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('user-interface-settings-view', 'Start minimized')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={startMinimized} onChange={setStartMinimized} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext(
              'user-interface-settings-view',
              'Show only the tray icon when the app starts.',
            )}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function AnimateMapSetting() {
  const animateMap = useSelector((state) => state.settings.guiSettings.animateMap);
  const { setAnimateMap } = useAppContext();

  return (
    <AriaInputGroup>
      <Cell.Container>
        <AriaLabel>
          <Cell.InputLabel>
            {messages.pgettext('user-interface-settings-view', 'Animate map')}
          </Cell.InputLabel>
        </AriaLabel>
        <AriaInput>
          <Cell.Switch isOn={animateMap} onChange={setAnimateMap} />
        </AriaInput>
      </Cell.Container>
      <Cell.CellFooter>
        <AriaDescription>
          <Cell.CellFooterText>
            {messages.pgettext('user-interface-settings-view', 'Animate map movements.')}
          </Cell.CellFooterText>
        </AriaDescription>
      </Cell.CellFooter>
    </AriaInputGroup>
  );
}

function LanguageButton() {
  const history = useHistory();
  const { getPreferredLocaleDisplayName } = useAppContext();
  const preferredLocale = useSelector((state) => state.settings.guiSettings.preferredLocale);
  const localeDisplayName = getPreferredLocaleDisplayName(preferredLocale);

  const navigate = useCallback(() => history.push(RoutePath.selectLanguage), [history]);

  return (
    <Cell.CellNavigationButton onClick={navigate}>
      <StyledCellIcon width={24} height={24} source="icon-language" />
      <Cell.Label>
        {
          // TRANSLATORS: Navigation button to the 'Language' settings view
          messages.pgettext('user-interface-settings-view', 'Language')
        }
      </Cell.Label>
      <Cell.SubText>{localeDisplayName}</Cell.SubText>
    </Cell.CellNavigationButton>
  );
}
