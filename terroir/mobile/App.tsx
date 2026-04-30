// SPDX-License-Identifier: AGPL-3.0-or-later
import 'react-native-gesture-handler';
import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { StatusBar } from 'expo-status-bar';

import { LoginScreen } from './src/screens/LoginScreen';
import { AgentProfileScreen } from './src/screens/AgentProfileScreen';
import { ProducersListScreen } from './src/screens/ProducersListScreen';
import { ProducerCreateScreen } from './src/screens/ProducerCreateScreen';
import { ParcelsListScreen } from './src/screens/ParcelsListScreen';
import { ParcelCreateScreen } from './src/screens/ParcelCreateScreen';
import { SyncStatusScreen } from './src/screens/SyncStatusScreen';
import { theme } from './src/theme';

export type RootStackParamList = {
  Login: undefined;
  Main: undefined;
  AgentProfile: undefined;
  ProducersList: undefined;
  ProducerCreate: undefined;
  ParcelsList: { producerId: string };
  ParcelCreate: { producerId: string };
  SyncStatus: undefined;
};

const Stack = createNativeStackNavigator<RootStackParamList>();
const Tab = createBottomTabNavigator();

/**
 * Bottom tabs : Producteurs / Parcelles d'un producteur (placeholder) / Profil.
 *
 * "Parcelles" requiert un producerId → en P1 on amène l'agent depuis la liste
 * des producteurs (tap sur ligne). L'onglet ouvre la même `ProducersListScreen`
 * sous un autre angle (filtre "avec parcelles" — TODO P2). Pour la simplicité
 * P1.E, le tab "Parcelles" pointe vers `ProducersList` aussi.
 */
function MainTabs() {
  return (
    <Tab.Navigator
      screenOptions={{
        headerStyle: { backgroundColor: theme.colors.primary },
        headerTintColor: theme.colors.onPrimary,
        headerTitleStyle: { fontWeight: '600' },
        tabBarActiveTintColor: theme.colors.primary,
        tabBarInactiveTintColor: '#757575',
      }}
    >
      <Tab.Screen
        name="ProducersList"
        component={ProducersListScreen}
        options={{ title: 'Producteurs', tabBarLabel: 'Producteurs' }}
      />
      <Tab.Screen
        name="AgentProfile"
        component={AgentProfileScreen}
        options={{ title: 'Profil', tabBarLabel: 'Profil' }}
      />
    </Tab.Navigator>
  );
}

export default function App() {
  return (
    <NavigationContainer>
      <StatusBar style="auto" />
      <Stack.Navigator
        initialRouteName="Login"
        screenOptions={{
          headerStyle: { backgroundColor: theme.colors.primary },
          headerTintColor: theme.colors.onPrimary,
          headerTitleStyle: { fontWeight: '600' },
        }}
      >
        <Stack.Screen
          name="Login"
          component={LoginScreen}
          options={{ title: 'TERROIR — Connexion', headerShown: false }}
        />
        <Stack.Screen
          name="Main"
          component={MainTabs}
          options={{ headerShown: false }}
        />
        <Stack.Screen
          name="ProducerCreate"
          component={ProducerCreateScreen}
          options={{ title: 'Nouveau producteur' }}
        />
        <Stack.Screen
          name="ParcelsList"
          component={ParcelsListScreen}
          options={{ title: 'Parcelles' }}
        />
        <Stack.Screen
          name="ParcelCreate"
          component={ParcelCreateScreen}
          options={{ title: 'Tracer une parcelle' }}
        />
        <Stack.Screen
          name="SyncStatus"
          component={SyncStatusScreen}
          options={{ title: 'Synchronisation' }}
        />
      </Stack.Navigator>
    </NavigationContainer>
  );
}
