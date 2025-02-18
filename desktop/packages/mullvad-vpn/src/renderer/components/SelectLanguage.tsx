import { useCallback, useEffect, useMemo, useRef } from 'react';
import styled from 'styled-components';

import { useAppContext } from '../../renderer/context';
import { messages } from '../../shared/gettext';
import { useHistory } from '../lib/history';
import { useSelector } from '../redux/store';
import { AppNavigationHeader } from './';
import { AriaInputGroup } from './AriaGroup';
import Selector, { SelectorItem } from './cell/Selector';
import { CustomScrollbarsRef } from './CustomScrollbars';
import { BackAction } from './KeyboardNavigation';
import { Layout, SettingsContainer } from './Layout';
import { NavigationContainer } from './NavigationContainer';
import { NavigationScrollbars } from './NavigationScrollbars';
import SettingsHeader, { HeaderTitle } from './SettingsHeader';

const StyledSelector = styled(Selector)({
  marginBottom: 0,
}) as typeof Selector;

export default function SelectLanguage() {
  const { pop } = useHistory();
  const { preferredLocale, preferredLocalesList, setPreferredLocale } = usePreferredLocale();
  const scrollView = useRef<CustomScrollbarsRef>(null);
  const selectedCellRef = useRef<HTMLButtonElement>(null);

  const selectLocale = useCallback(
    async (locale: string) => {
      await setPreferredLocale(locale);
      pop();
    },
    [pop, setPreferredLocale],
  );

  const scrollToSelectedCell = () => {
    const ref = selectedCellRef.current;
    const view = scrollView.current;
    if (view && ref) {
      if (ref instanceof HTMLElement) {
        view.scrollToElement(ref, 'middle');
      }
    }
  };

  useEffect(() => {
    scrollToSelectedCell();
  }, []);

  return (
    <BackAction action={pop}>
      <Layout>
        <SettingsContainer>
          <NavigationContainer>
            <AppNavigationHeader
              title={
                // TRANSLATORS: Title label in navigation bar
                messages.pgettext('select-language-nav', 'Select language')
              }
            />

            <NavigationScrollbars ref={scrollView}>
              <SettingsHeader>
                <HeaderTitle>
                  {messages.pgettext('select-language-nav', 'Select language')}
                </HeaderTitle>
              </SettingsHeader>
              <AriaInputGroup>
                <StyledSelector
                  title=""
                  value={preferredLocale}
                  items={preferredLocalesList}
                  onSelect={selectLocale}
                  selectedCellRef={selectedCellRef}
                />
              </AriaInputGroup>
            </NavigationScrollbars>
          </NavigationContainer>
        </SettingsContainer>
      </Layout>
    </BackAction>
  );
}

function usePreferredLocale() {
  const preferredLocale = useSelector((state) => state.settings.guiSettings.preferredLocale);

  const { getPreferredLocaleList, setPreferredLocale } = useAppContext();

  const preferredLocalesList: SelectorItem<string>[] = useMemo(() => {
    return [...getPreferredLocaleList().map(({ name, code }) => ({ label: name, value: code }))];
  }, [getPreferredLocaleList]);

  return { preferredLocale, preferredLocalesList, setPreferredLocale };
}
