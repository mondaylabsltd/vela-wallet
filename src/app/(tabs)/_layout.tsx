import { Tabs } from 'expo-router';
import React from 'react';

/**
 * Layout A is a single home screen with a custom WaveDock — there is no bottom
 * tab bar. We keep the Tabs navigator (so existing /wallet, /connect, /settings,
 * /dapps routes and deep links keep resolving) but hide the bar. Settings/Connect
 * are reached from the home header / scanner and provide their own close control.
 */
export default function TabLayout() {
  return (
    <Tabs screenOptions={{ headerShown: false, tabBarStyle: { display: 'none' } }}>
      <Tabs.Screen name="wallet" />
      <Tabs.Screen name="connect" />
      <Tabs.Screen name="dapps" options={{ href: null }} />
      <Tabs.Screen name="settings" />
    </Tabs>
  );
}
