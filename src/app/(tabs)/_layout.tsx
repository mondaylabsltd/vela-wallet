import { Tabs } from 'expo-router';
import React from 'react';
import { Platform } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { VelaColor } from '@/constants/theme';
import { Wallet, Settings } from 'lucide-react-native';

export default function TabLayout() {
  const insets = useSafeAreaInsets();
  // Android nav bar: use safe area bottom inset; iOS handled natively
  const bottomPadding = Platform.OS === 'android' ? insets.bottom + 6 : 8;
  const tabHeight = Platform.OS === 'android' ? 56 + insets.bottom : 60;

  return (
    <Tabs
      screenOptions={{
        headerShown: false,
        tabBarActiveTintColor: VelaColor.accent,
        tabBarInactiveTintColor: VelaColor.textTertiary,
        tabBarStyle: {
          backgroundColor: VelaColor.bgCard,
          borderTopColor: VelaColor.border,
          paddingBottom: bottomPadding,
          height: tabHeight,
        },
      }}
    >
      <Tabs.Screen
        name="wallet"
        options={{
          title: 'Wallet',
          tabBarIcon: ({ color, size }) => <Wallet size={size ?? 22} color={color} />,
        }}
      />
      <Tabs.Screen
        name="dapps"
        options={{
          href: null, // hidden in v1
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: 'Settings',
          tabBarIcon: ({ color, size }) => <Settings size={size ?? 22} color={color} />,
        }}
      />
    </Tabs>
  );
}
