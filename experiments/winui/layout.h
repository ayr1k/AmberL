// SPDX-License-Identifier: MPL-2.0
constexpr auto layout = LR"XAML(
<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Margin="20" RowSpacing="10" MinWidth="800" MinHeight="620">
 <Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="*"/><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="82"/><RowDefinition Height="Auto"/><RowDefinition Height="125"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>
 <Grid ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><TextBlock Text="AmberL / 游戏库 Library" FontSize="24" FontWeight="SemiBold"/><Button x:Name="About" Grid.Column="1" Content="关于 / About"/></Grid>
 <TextBlock Grid.Row="1" Text="WinUI 3 比较样机 · 只读核心扫描 · 非最终界面"/>
 <TextBlock Grid.Row="2" Text="测试库路径 / Test library path"/>
 <Grid Grid.Row="3" ColumnSpacing="10"><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><TextBox x:Name="Path" AutomationProperties.Name="测试库路径 Test library path"/><Button x:Name="Scan" Grid.Column="1" Content="扫描 / Scan"/><Button x:Name="Cancel" Grid.Column="2" Content="取消 / Cancel"/></Grid>
 <TextBox x:Name="Filter" Grid.Row="4" PlaceholderText="筛选名称、版本、加载器 / Filter" AutomationProperties.Name="筛选 Filter"/>
 <Grid Grid.Row="5" ColumnSpacing="10"><Grid.ColumnDefinitions><ColumnDefinition Width="3*"/><ColumnDefinition Width="2*"/></Grid.ColumnDefinitions><ListView x:Name="List" SelectionMode="Single" AutomationProperties.Name="游戏库条目 Library entries"/><TextBox x:Name="Detail" Grid.Column="1" IsReadOnly="True" AcceptsReturn="True" TextWrapping="Wrap" AutomationProperties.Name="条目详情 Instance details"/></Grid>
 <TextBlock x:Name="Status" Grid.Row="6" TextWrapping="Wrap"/>
 <TextBlock Grid.Row="7" Text="中文输入与编辑 / IME, selection, undo and keyboard test"/>
 <TextBox Grid.Row="8" AcceptsReturn="True" TextWrapping="Wrap" PlaceholderText="在这里测试中文预编辑、选择、撤销和键盘导航。" AutomationProperties.Name="中文输入测试 Chinese input test"/>
 <Grid Grid.Row="9" ColumnSpacing="10"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="*"/></Grid.ColumnDefinitions><Button x:Name="StartTask" Content="模拟任务 / Test task"/><Button x:Name="CancelTask" Grid.Column="1" Content="取消任务 / Cancel task"/><ProgressBar x:Name="Progress" Grid.Column="2" Minimum="0" Maximum="100" VerticalAlignment="Center"/></Grid>
 <TextBox x:Name="Log" Grid.Row="10" IsReadOnly="True" AcceptsReturn="True" TextWrapping="NoWrap" AutomationProperties.Name="有界模拟日志 Bounded synthetic log"/>
 <TextBlock x:Name="AboutText" Grid.Row="11" Visibility="Collapsed" Text="Ambrum Rust 核心 · C++/WinRT + WinUI 3 · 单进程技术比较&#10;MPL-2.0 source / Microsoft Windows App SDK notices included" TextWrapping="Wrap"/>
</Grid>)XAML";
