import { Tabs } from 'expo-router';
import React from 'react';
import { Platform } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { color } from '@/constants/theme';
import { Wallet, Settings } from 'lucide-react-native';

export default function TabLayout() {
  const insets = useSafeAreaInsets();
  const bottomPadding = Platform.OS === 'android' ? insets.bottom + 6 : 8;
  const tabHeight = Platform.OS === 'android' ? 56 + insets.bottom : 60;

  return (
    <Tabs
      screenOptions={{
        headerShown: false,
        tabBarActiveTintColor: color.accent.base,
        tabBarInactiveTintColor: color.fg.subtle,
        tabBarStyle: {
          backgroundColor: color.bg.raised,
          borderTopColor: color.border.base,
          paddingBottom: bottomPadding,
          height: tabHeight,
        },
      }}
    >
      <Tabs.Screen
        name="wallet"
        options={{
          title: 'Wallet',
          tabBarIcon: ({ color: c, size }) => <Wallet size={size ?? 22} color={c} />,
        }}
      />
      <Tabs.Screen
        name="dapps"
        options={{
          href: null,
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: 'Settings',
          tabBarIcon: ({ color: c, size }) => <Settings size={size ?? 22} color={c} />,
        }}
      />
    </Tabs>
  );
}
